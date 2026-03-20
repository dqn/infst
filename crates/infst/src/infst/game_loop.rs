//! Game loop and state handling for INFST
//!
//! This module contains the main tracking loop and game state handling methods.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use chrono::Utc;
use tracing::{debug, error, info, warn};

use crate::chart::{
    ChartInfo, Difficulty, SongInfo, fetch_song_by_id, fetch_song_database_from_memory_scan,
    get_unlock_states,
};
use crate::config::{check_version_match, find_game_version, polling, retry};
use crate::error::Result;
use crate::export::format_play_data_console;
use crate::play::{AssistType, GameState, PlayData, PlayType, RawSettings, Settings};
use crate::process::layout::{judge, play, settings, timing};
use crate::process::{MemoryReader, ProcessHandle, ReadMemory};
use crate::score::{Grade, Judge, Lamp, PlayerJudge, RawJudgeData, ScoreMap};

use super::Infst;

/// Read a value from memory with a default on error.
///
/// This helper simplifies error handling for non-critical reads.
fn read_with_default<T, F>(f: F, default: T, context: &str) -> T
where
    F: FnOnce() -> Result<T>,
{
    match f() {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to read {}: {}", context, e);
            default
        }
    }
}

/// Check if memory is accessible with retry logic.
///
/// Uses exponential backoff and checks process liveness between retries.
fn verify_memory_access(reader: &MemoryReader, process: &ProcessHandle) -> bool {
    for attempt in 0..retry::MAX_READ_RETRIES {
        match reader.read_bytes(process.base_address, 4) {
            Ok(_) => return true,
            Err(e) => {
                // Re-check process status before retrying
                if !process.is_alive() {
                    debug!("Process terminated during retry: {}", e);
                    return false;
                }

                if attempt < retry::MAX_READ_RETRIES - 1 {
                    let delay = retry::RETRY_DELAYS_MS[attempt as usize];
                    debug!(
                        "Memory read failed (attempt {}/{}, retry in {}ms): {}",
                        attempt + 1,
                        retry::MAX_READ_RETRIES,
                        delay,
                        e
                    );
                    thread::sleep(Duration::from_millis(delay));
                } else {
                    debug!(
                        "Memory read failed after {} retries: {}",
                        retry::MAX_READ_RETRIES,
                        e
                    );
                }
            }
        }
    }
    false
}

impl Infst {
    /// Run the main tracking loop
    ///
    /// The `shutdown_requested` flag is checked each iteration to allow graceful shutdown via Ctrl+C.
    /// When `shutdown_requested` is `true`, the loop exits.
    pub fn run(&mut self, process: &ProcessHandle, shutdown_requested: &AtomicBool) -> Result<()> {
        let reader = MemoryReader::new(process);
        let mut last_state = GameState::Unknown;

        debug!("Starting tracker loop...");

        // Start TSV session (re-initialize to reset state for new run)
        self.session_manager = crate::session::SessionManager::new(&self.config.session_dir);
        match self.session_manager.start_tsv_session() {
            Ok(path) => debug!("Started TSV session at {:?}", path),
            Err(e) => warn!("Failed to start TSV session: {}", e),
        }

        loop {
            // Check for shutdown signal
            if shutdown_requested.load(Ordering::SeqCst) {
                debug!("Shutdown signal received, exiting tracker loop");
                break;
            }

            // Step 1: Fast check if process is still alive via exit code
            if !process.is_alive() {
                debug!("Process terminated (exit code check)");
                break;
            }

            // Step 2: Verify memory access with retry mechanism (exponential backoff)
            if !verify_memory_access(&reader, process) {
                break;
            }

            // Detect game state
            let current_state = self.detect_game_state(&reader);

            if current_state != last_state {
                debug!("State changed: {:?} -> {:?}", last_state, current_state);
                // Clear pending result polling when leaving ResultScreen
                if last_state == GameState::ResultScreen {
                    self.result_poll_pending = false;
                    self.result_poll_ticks = 0;
                    self.pending_result_fingerprint = None;
                }
                self.handle_state_change(&reader, last_state, current_state)?;
                last_state = current_state;
            }

            // V3: Poll for uncaptured results while in ResultScreen
            // This handles the case where Playing state was not detected
            if current_state == GameState::ResultScreen && self.result_poll_pending {
                self.poll_pending_result(&reader);
            }

            thread::sleep(Duration::from_millis(timing::GAME_STATE_POLL_INTERVAL_MS));
        }

        Ok(())
    }

    fn detect_game_state(&mut self, reader: &MemoryReader) -> GameState {
        let state_marker_1 = read_with_default(
            || reader.read_i32(self.offsets.judge_data + judge::STATE_MARKER_1),
            0,
            "state_marker_1",
        );
        let state_marker_2 = read_with_default(
            || reader.read_i32(self.offsets.judge_data + judge::STATE_MARKER_2),
            0,
            "state_marker_2",
        );
        let song_select_marker = read_with_default(
            || {
                self.offsets
                    .play_settings
                    .checked_sub(settings::SONG_SELECT_MARKER)
                    .map_or(Ok(0), |addr| reader.read_i32(addr))
            },
            0,
            "song_select_marker",
        );

        // Temporary diagnostic: log raw markers when they change
        let last = self.state_detector.last_state();
        let markers = (state_marker_1, state_marker_2, song_select_marker);
        if self.last_markers != markers {
            debug!(
                "Markers: m1={} m2={} ss={} (last_state={:?})",
                state_marker_1, state_marker_2, song_select_marker, last
            );
            self.last_markers = markers;
        }

        self.state_detector
            .detect(state_marker_1, state_marker_2, song_select_marker)
    }

    fn handle_state_change(
        &mut self,
        reader: &MemoryReader,
        old_state: GameState,
        new_state: GameState,
    ) -> Result<()> {
        match new_state {
            GameState::ResultScreen => self.handle_result_screen(reader, old_state),
            GameState::SongSelect => self.handle_song_select(reader),
            GameState::Playing => self.handle_playing(reader),
            GameState::Unknown => {}
        }
        Ok(())
    }

    /// Handle transition to result screen
    fn handle_result_screen(&mut self, reader: &MemoryReader, from_state: GameState) {
        // Skip result polling when from_state is Unknown (app just started) -
        // there's no real play result to capture.
        if from_state == GameState::Unknown {
            debug!("Unknown -> ResultScreen (startup), skipping result polling");
            return;
        }

        // In V3, Playing markers may not fire, causing SongSelect -> ResultScreen
        // without detecting Playing. Instead of skipping, enable continuous polling
        // in the main loop. Data will be captured once it stabilizes (same fingerprint
        // seen twice in a row), with dedup preventing stale data capture.
        if from_state == GameState::SongSelect {
            info!("SongSelect -> ResultScreen (Playing not detected), enabling result polling");
            self.result_poll_pending = true;
            self.result_poll_ticks = 0;
            self.pending_result_fingerprint = None;
            return;
        }
        info!("Detected result screen, waiting for data...");

        // Initial delay to allow game data to settle (matching C# implementation)
        // This prevents race conditions where judge data updates before play data
        thread::sleep(Duration::from_millis(polling::RESULT_INITIAL_DELAY_MS));

        // Poll until play data becomes available (exponential backoff)
        let mut ever_saw_valid_lamp = false;
        for (attempt, &delay) in polling::POLL_DELAYS_MS.iter().enumerate() {
            thread::sleep(Duration::from_millis(delay));

            match self.fetch_play_data(reader) {
                Ok((play_data, notes_from_current_song)) => {
                    // Verify data looks valid (non-zero total notes)
                    let total_notes = play_data
                        .judge
                        .pgreat
                        .saturating_add(play_data.judge.great)
                        .saturating_add(play_data.judge.good)
                        .saturating_add(play_data.judge.bad)
                        .saturating_add(play_data.judge.poor);

                    // Validate song_id and difficulty match current_playing (if available)
                    let chart_valid = match self.current_playing {
                        Some((expected_id, expected_diff)) => {
                            play_data.chart.song_id == expected_id
                                && play_data.chart.difficulty == expected_diff
                        }
                        None => true, // No reference, accept any
                    };

                    // Lamp must be at least Failed when notes exist (NoPlay means data not yet written)
                    let lamp_valid = play_data.lamp >= Lamp::Failed;
                    if lamp_valid {
                        ever_saw_valid_lamp = true;
                    }

                    debug!(
                        "Attempt {}: song_id={}, total_notes={}, chart_valid={}, lamp={}, lamp_valid={}, judge: P={} G={} Go={} B={} Po={}",
                        attempt + 1,
                        play_data.chart.song_id,
                        total_notes,
                        chart_valid,
                        play_data.lamp,
                        lamp_valid,
                        play_data.judge.pgreat,
                        play_data.judge.great,
                        play_data.judge.good,
                        play_data.judge.bad,
                        play_data.judge.poor
                    );

                    if total_notes > 0 && chart_valid && lamp_valid {
                        // Dedup: skip if this is the same result we already captured
                        let fingerprint = (
                            play_data.chart.song_id,
                            play_data.chart.difficulty as u8,
                            play_data.ex_score,
                        );
                        if self.last_result_fingerprint == Some(fingerprint) {
                            debug!("Skipping duplicate result: {:?}", fingerprint);
                            return;
                        }

                        // Write back authoritative notes to song_db only after
                        // confirming this is not a duplicate result.
                        if notes_from_current_song {
                            self.write_back_notes(&play_data);
                        }

                        info!(
                            "Play result captured: {} ({}) - EX: {}",
                            play_data.chart.title, play_data.chart.song_id, play_data.ex_score
                        );
                        self.last_result_fingerprint = Some(fingerprint);
                        self.process_play_result(&play_data);
                        self.current_playing = None; // Clear after processing
                        return;
                    }
                    // Data not ready yet, continue polling
                    if attempt == polling::POLL_DELAYS_MS.len() - 1 {
                        debug!(
                            "Play data validation failed after {} attempts (notes={}, chart_valid={}, lamp_valid={})",
                            polling::POLL_DELAYS_MS.len(),
                            total_notes,
                            chart_valid,
                            lamp_valid,
                        );
                    }
                }
                Err(e) => {
                    if attempt == polling::POLL_DELAYS_MS.len() - 1 {
                        debug!(
                            "Failed to fetch play data after {} attempts: {}",
                            polling::POLL_DELAYS_MS.len(),
                            e
                        );
                    }
                }
            }
        }

        // Only fall back to continuous polling if we saw valid lamp at some point.
        // If lamp was always NoPlay, this is a false Playing->ResultScreen transition
        // caused by state marker jitter, not a real play result.
        if ever_saw_valid_lamp {
            debug!("Initial polling incomplete, enabling continuous polling");
            self.result_poll_pending = true;
            self.result_poll_ticks = 0;
            self.pending_result_fingerprint = None;
        } else {
            debug!("No valid lamp during initial polling (false transition), skipping");
            // Do NOT clear current_playing here. This is a false transition, so the
            // cross-validation reference should be preserved for the next real result.
            // Clearing it would cause the next result to match via None => true
            // (accept any chart), bypassing cross-validation entirely.
        }
    }

    /// Poll for result data while in ResultScreen state (V3 fallback)
    ///
    /// Called from the main loop when `result_poll_pending` is true.
    /// Uses a stability check: data must produce the same fingerprint on two
    /// consecutive checks (~1 second apart) to confirm the result is final
    /// and not mid-play accumulating judge counts.
    fn poll_pending_result(&mut self, reader: &MemoryReader) {
        self.result_poll_ticks += 1;

        // Throttle: check every 10 ticks (~1 second at 100ms polling)
        if !self.result_poll_ticks.is_multiple_of(10) {
            return;
        }

        let (play_data, notes_from_current_song) = match self.fetch_play_data(reader) {
            Ok(data) => data,
            Err(_) => {
                self.pending_result_fingerprint = None;
                return;
            }
        };

        let total_notes = play_data
            .judge
            .pgreat
            .saturating_add(play_data.judge.great)
            .saturating_add(play_data.judge.good)
            .saturating_add(play_data.judge.bad)
            .saturating_add(play_data.judge.poor);

        let lamp_valid = play_data.lamp >= Lamp::Failed;

        if total_notes == 0 || !lamp_valid {
            self.pending_result_fingerprint = None;
            return;
        }

        let fingerprint = (
            play_data.chart.song_id,
            play_data.chart.difficulty as u8,
            play_data.ex_score,
        );

        // Dedup: skip if this is the same result we already captured
        if self.last_result_fingerprint == Some(fingerprint) {
            return;
        }

        // Stability check: fingerprint must match the previous poll.
        // During play, judge counts change constantly so fingerprints differ.
        // On the actual result screen, data is stable.
        match self.pending_result_fingerprint {
            Some(prev) if prev == fingerprint => {
                // Write back authoritative notes to song_db only after
                // confirming this is not a duplicate result.
                if notes_from_current_song {
                    self.write_back_notes(&play_data);
                }

                // Stable for 2 consecutive checks - capture the result
                info!(
                    "Play result captured (polling): {} ({}) - EX: {}",
                    play_data.chart.title, play_data.chart.song_id, play_data.ex_score
                );
                self.last_result_fingerprint = Some(fingerprint);
                self.process_play_result(&play_data);
                self.current_playing = None;
                self.pending_result_fingerprint = None;
                self.result_poll_pending = false;
            }
            _ => {
                // First time or data changed, remember and recheck next tick
                self.pending_result_fingerprint = Some(fingerprint);
            }
        }
    }

    /// Process and save play result data
    fn process_play_result(&mut self, play_data: &PlayData) {
        // Get personal best for comparison (before updating score_map)
        let personal_best = self.game_data.score_map.get(play_data.chart.song_id);

        // Print detailed play data to console (with PB comparison)
        println!("{}", format_play_data_console(play_data, personal_best));

        // Update score_map with current play data so the export reflects this play
        self.update_score_map(play_data);

        // Save to session files
        self.save_session_data(play_data);

        // Send to API (non-blocking)
        self.send_lamp_to_api(play_data);

        // Export scores and git commit/push (non-blocking)
        self.export_and_git_push(play_data);
    }

    /// Update score_map with the current play's data
    ///
    /// Ensures the JSON export includes the latest play result immediately,
    /// rather than waiting for a full reload from game memory on song select.
    fn update_score_map(&mut self, play_data: &PlayData) {
        let entry = self
            .game_data
            .score_map
            .get_or_insert(play_data.chart.song_id);
        let diff = play_data.chart.difficulty;
        let diff_index = diff as usize;

        let old_lamp = entry.get_lamp(diff);
        let old_score = entry.get_score(diff);
        let old_miss = entry.miss_count[diff_index];
        let mut updated = false;

        // Update lamp (keep best)
        if play_data.lamp > old_lamp {
            entry.set_lamp(diff, play_data.lamp);
            updated = true;
        }

        // Update EX score (keep best)
        if play_data.ex_score > old_score {
            entry.set_score(diff, play_data.ex_score);
            updated = true;
        }

        // Update miss count (keep lowest)
        if play_data.miss_count_valid() {
            let current_miss = play_data.miss_count();
            match old_miss {
                Some(best) if current_miss < best => {
                    entry.miss_count[diff_index] = Some(current_miss);
                    updated = true;
                }
                None => {
                    entry.miss_count[diff_index] = Some(current_miss);
                    updated = true;
                }
                _ => {}
            }
        }

        info!(
            "score_map update: song={} {} | lamp: {}→{} | ex: {}→{} | miss: {:?}→{} (valid={}) | changed={}",
            play_data.chart.song_id,
            diff.short_name(),
            old_lamp,
            play_data.lamp,
            old_score,
            play_data.ex_score,
            old_miss,
            play_data.miss_count(),
            play_data.miss_count_valid(),
            updated,
        );
    }

    /// Export full score data to JSON and git commit/push in a background thread
    fn export_and_git_push(&mut self, play_data: &PlayData) {
        let Some(ref git_config) = self.config.git_config else {
            return;
        };

        let file_path = git_config.repo_path.join(&git_config.file_name);

        // Export JSON to the git repo
        if let Err(e) = crate::export::export_tracker_json(
            &file_path,
            &self.game_data.song_db,
            &self.game_data.unlock_state,
            &self.game_data.score_map,
        ) {
            error!("Failed to export scores for git: {}", e);
            return;
        }

        // Build commit label and message
        let label = format!(
            "{} ({}) - {} EX:{}",
            play_data.chart.title,
            play_data.chart.difficulty.short_name(),
            play_data.lamp.short_name(),
            play_data.ex_score,
        );
        let message = format!("Update scores: {}", label);

        let repo_path = git_config.repo_path.clone();
        let file_name = git_config.file_name.clone();

        thread::spawn(move || {
            if let Err(e) = crate::git::add_commit_push(&repo_path, &file_name, &message, &label) {
                error!("Git commit/push failed: {}", e);
            }
        });
    }

    /// Send lamp data to the API endpoint in a background thread
    #[cfg(feature = "api")]
    fn send_lamp_to_api(&self, play_data: &PlayData) {
        let Some(ref api_config) = self.config.api_config else {
            return;
        };

        // Only level 11/12 charts are synced to the web API.
        if !matches!(play_data.chart.level, 11 | 12) {
            return;
        }

        // Look up personal best from score_map (already updated by update_score_map)
        // to send the best values rather than the current play's values, which may
        // be worse than the personal best.
        let best = self.game_data.score_map.get(play_data.chart.song_id);
        let diff = play_data.chart.difficulty;

        let best_ex = best.map_or(play_data.ex_score, |s| {
            s.get_score(diff).max(play_data.ex_score)
        });
        let best_lamp = best.map_or(play_data.lamp, |s| s.get_lamp(diff).max(play_data.lamp));

        let current_miss = if play_data.miss_count_valid() {
            Some(play_data.miss_count())
        } else {
            None
        };
        let best_miss = match (best.and_then(|s| s.miss_count[diff as usize]), current_miss) {
            (Some(pb), Some(cur)) => Some(pb.min(cur)),
            (Some(pb), None) => Some(pb),
            (None, Some(cur)) => Some(cur),
            (None, None) => None,
        };

        let req = LampRequest {
            endpoint: api_config.endpoint.clone(),
            token: api_config.token.clone(),
            song_id: play_data.chart.song_id,
            title: play_data.chart.title.trim().to_string(),
            difficulty: play_data.chart.difficulty.short_name().to_string(),
            lamp: best_lamp.short_name().to_string(),
            ex_score: best_ex,
            miss_count: best_miss,
        };

        thread::spawn(move || {
            if let Err(e) = send_lamp_request(&req) {
                warn!("Failed to send lamp to API: {}", e);
            }
        });
    }

    #[cfg(not(feature = "api"))]
    fn send_lamp_to_api(&self, _play_data: &PlayData) {}

    /// Save play data to session file (TSV)
    fn save_session_data(&mut self, play_data: &PlayData) {
        debug!(
            "Saving session data: song_id={}, title={}, ex_score={}",
            play_data.chart.song_id, play_data.chart.title, play_data.ex_score
        );

        if self.session_manager.current_session_path().is_none() {
            warn!("No active TSV session, attempting to start one...");
            if let Err(e) = self.session_manager.start_tsv_session() {
                error!("Failed to start TSV session: {}", e);
                return;
            }
        }

        match self.session_manager.append_tsv_row(play_data) {
            Ok(()) => {
                if let Some(path) = self.session_manager.current_session_path() {
                    debug!("Successfully wrote to session file: {:?}", path);
                }
            }
            Err(e) => error!("Failed to append TSV row: {}", e),
        }
    }

    /// Handle transition to song select screen
    fn handle_song_select(&mut self, reader: &MemoryReader) {
        // Re-scan for newly loaded songs (handles lazy loading)
        self.rescan_song_database(reader);

        // Poll unlock state changes
        self.poll_unlock_changes(reader);

        // Reload score map to reflect latest play results
        self.reload_score_map(reader);

        // Export tracker file if auto-export is enabled
        if self.config.auto_export {
            let tracker_path = self.config.tracker_path.clone();
            if let Err(e) = self.export_tracker_tsv(&tracker_path) {
                error!("Failed to export tracker file: {}", e);
            }
        }
    }

    /// Reload score map from memory
    ///
    /// Called when new songs are discovered to ensure score comparisons
    /// work for all known songs.
    fn reload_score_map(&mut self, reader: &MemoryReader) {
        match ScoreMap::load_from_memory(reader, self.offsets.data_map, &self.game_data.song_db) {
            Ok(map) => {
                info!("Reloaded score map: {} entries", map.len());
                self.game_data.score_map = map;
            }
            Err(e) => warn!("Failed to reload score map: {}", e),
        }
    }

    /// Re-scan memory for newly loaded songs
    ///
    /// This handles lazy loading in newer INFINITAS versions where songs are
    /// only loaded into memory when scrolled to in the song select screen.
    fn rescan_song_database(&mut self, reader: &MemoryReader) {
        let entry_stride = if self.offsets.song_entry_size > 0 {
            self.offsets.song_entry_size
        } else {
            SongInfo::MEMORY_SIZE
        };
        let Some(scan_size) = entry_stride.checked_mul(5000) else {
            warn!(
                "Entry stride overflow: {} * 5000, skipping rescan",
                entry_stride
            );
            return;
        };
        let layout = self.offsets.effective_layout();
        let scan_result = fetch_song_database_from_memory_scan(
            reader,
            self.offsets.song_db_address(),
            scan_size,
            entry_stride,
            &layout,
        );

        let mut new_songs = 0usize;
        for (song_id, song) in scan_result {
            if let std::collections::hash_map::Entry::Vacant(e) =
                self.game_data.song_db.entry(song_id)
            {
                debug!(
                    "Discovered new song via rescan: {} ({})",
                    song.title, song_id
                );
                e.insert(song);
                new_songs += 1;
            }
        }

        if new_songs > 0 {
            info!(
                "Re-scan discovered {} new songs (total: {})",
                new_songs,
                self.game_data.song_db.len()
            );
        }
    }

    /// Handle transition to playing state
    ///
    /// Captures current chart selection when entering Playing state.
    /// This is used for cross-validation on ResultScreen to ensure
    /// we're reading the correct play data.
    fn handle_playing(&mut self, reader: &MemoryReader) {
        match self.fetch_current_chart(reader) {
            Ok((song_id, difficulty)) => {
                debug!(
                    "Entering Playing state: song_id={}, difficulty={:?}",
                    song_id, difficulty
                );
                self.current_playing = Some((song_id, difficulty));
            }
            Err(e) => {
                warn!("Failed to fetch current chart on Playing: {}", e);
                // Keep previous value if any, or None
            }
        }
    }

    /// Poll for unlock state changes
    fn poll_unlock_changes(&mut self, reader: &MemoryReader) {
        if self.game_data.song_db.is_empty() {
            return;
        }

        // Read current unlock state
        let current_state =
            match get_unlock_states(reader, self.offsets.unlock_data, &self.game_data.song_db) {
                Ok(state) => state,
                Err(e) => {
                    error!("Failed to read unlock state: {}", e);
                    return;
                }
            };

        // Detect changes
        let changes =
            crate::chart::detect_unlock_changes(&self.game_data.unlock_state, &current_state);

        if !changes.is_empty() {
            debug!("Detected {} unlock state changes", changes.len());
        }

        // Update current unlock state
        self.game_data.unlock_state = current_state;
    }

    /// Fetch current chart selection from memory
    ///
    /// Used during Playing state to capture what chart is being played,
    /// enabling cross-validation when reading play data on ResultScreen.
    fn fetch_current_chart(&self, reader: &MemoryReader) -> Result<(u32, Difficulty)> {
        let song_id = reader.read_i32(self.offsets.current_song)?;
        let diff = reader.read_i32(self.offsets.current_song + 4)?;

        if !(1000..=50000).contains(&song_id) {
            return Err(crate::error::Error::invalid_game_state(
                "song_id in 1000..=50000",
                format!("{song_id}"),
            ));
        }

        let difficulty = Difficulty::from_u8(diff as u8).ok_or_else(|| {
            crate::error::Error::invalid_game_state("difficulty in 0..=9", format!("{diff}"))
        })?;

        Ok((song_id as u32, difficulty))
    }

    /// Fetch play data from memory.
    ///
    /// Returns `(PlayData, notes_from_current_song)`. When the second element is
    /// `true`, the caller should call `write_back_notes` after confirming the
    /// result is not a duplicate.
    fn fetch_play_data(&mut self, reader: &MemoryReader) -> Result<(PlayData, bool)> {
        // Read data in same order as C# implementation:
        // 1. Judge data first (updates earliest on result screen)
        // 2. Settings
        // 3. PlayData last (song_id, difficulty, lamp)
        // This ordering ensures we get consistent data when transitioning to result screen,
        // since judge data updates before play data in the game.
        let judge = self.fetch_judge_data(reader)?;
        let settings = self.fetch_settings(reader, judge.play_type)?;

        // Read basic play data (after judge/settings to match C# timing)
        let song_id_raw = reader.read_i32(self.offsets.play_data + play::SONG_ID)?;
        let difficulty_val = reader.read_i32(self.offsets.play_data + play::DIFFICULTY)?;
        let lamp_val = reader.read_i32(self.offsets.play_data + play::LAMP)?;

        // Validate ranges before narrowing casts (same pattern as fetch_current_chart)
        if !(1000..=50000).contains(&song_id_raw) {
            return Err(crate::error::Error::invalid_game_state(
                "play_data song_id in 1000..=50000",
                format!("{song_id_raw}"),
            ));
        }
        let song_id = song_id_raw as u32;

        if !(0..=9).contains(&difficulty_val) {
            return Err(crate::error::Error::invalid_game_state(
                "play_data difficulty in 0..=9",
                format!("{difficulty_val}"),
            ));
        }
        let difficulty = Difficulty::from_u8(difficulty_val as u8).ok_or_else(|| {
            crate::error::Error::invalid_game_state(
                "play_data difficulty in 0..=9",
                format!("{difficulty_val}"),
            )
        })?;

        if !(0..=7).contains(&lamp_val) {
            return Err(crate::error::Error::invalid_game_state(
                "play_data lamp in 0..=7",
                format!("{lamp_val}"),
            ));
        }
        let lamp = Lamp::from_u8(lamp_val as u8).ok_or_else(|| {
            crate::error::Error::invalid_game_state(
                "play_data lamp in 0..=7",
                format!("{lamp_val}"),
            )
        })?;

        // Calculate EX score
        let ex_score = judge.ex_score();
        let data_available =
            !settings.h_ran && !settings.battle && settings.assist == AssistType::Off;

        let mut chart = self.create_chart_info_dynamic(reader, song_id, difficulty);

        debug!(
            "Chart: song_id={} diff={:?} total_notes={} all_notes={:?}",
            chart.song_id,
            chart.difficulty,
            chart.total_notes,
            self.game_data.song_db.get(&song_id).map(|s| s.total_notes),
        );

        // Calculate grade.
        // V3: entry offset 0x378 contains BPM, not total_notes. Read the
        // actual total_notes from CurrentSong + 0x10 in memory.
        // Fall back to entry data if available, then to judge note count
        // as last resort.
        let chart_notes = self.read_current_chart_notes(reader);
        let judge_notes = judge
            .pgreat
            .saturating_add(judge.great)
            .saturating_add(judge.good)
            .saturating_add(judge.bad)
            .saturating_add(judge.poor);

        // V3: entry offset 0x378 contains BPM, not total_notes. Detect this
        // by checking if all non-zero values in the song's total_notes array
        // are identical -- real note counts vary across difficulties, BPM does not.
        let entry_notes_likely_bpm = self
            .game_data
            .song_db
            .get(&song_id)
            .map(|song| {
                let non_zero: Vec<u32> = song
                    .total_notes
                    .iter()
                    .copied()
                    .filter(|&n| n > 0)
                    .collect();
                non_zero.len() >= 2 && non_zero.iter().all(|&n| n == non_zero[0])
            })
            .unwrap_or(false);

        // Track whether the notes source is authoritative (from game memory)
        // so we only write back reliable values to song_db.
        // judge_notes is NOT authoritative: partial plays (premature end) yield
        // fewer notes than the chart actually has.
        let (effective_notes, notes_from_current_song) = if chart_notes > 0 {
            (chart_notes, true)
        } else if chart.total_notes > 0
            && !entry_notes_likely_bpm
            && (ex_score as u64) <= (chart.total_notes as u64) * 2
        {
            (chart.total_notes, false) // not from CurrentSong memory read, skip writeback
        } else if judge_notes > 0 {
            (judge_notes, false) // unreliable for partial plays, do NOT write back
        } else {
            (0, false)
        };

        // Update chart's total_notes for grade calculation
        if effective_notes > 0 {
            chart.total_notes = effective_notes;
        }

        let grade = if effective_notes > 0 {
            PlayData::calculate_grade(ex_score, effective_notes)
        } else {
            Grade::NoPlay
        };

        Ok((
            PlayData {
                timestamp: Utc::now(),
                chart,
                ex_score,
                grade,
                lamp,
                judge,
                settings,
                data_available,
            },
            notes_from_current_song,
        ))
    }

    /// Write back authoritative total_notes from CurrentSong to song_db.
    ///
    /// Only call this after confirming the play result is NOT a duplicate,
    /// so that song_db is not mutated for discarded results.
    fn write_back_notes(&mut self, play_data: &PlayData) {
        let song_id = play_data.chart.song_id;
        let diff_index = play_data.chart.difficulty as usize;
        if let Some(song) = self.game_data.song_db.get_mut(&song_id) {
            song.total_notes[diff_index] = play_data.chart.total_notes;
        }
    }

    /// Create chart info from song database, dynamically loading from memory if not found.
    ///
    /// In V3, game_id (from CurrentSong/PlayData) may differ from internal_id
    /// (in the entry table). This method first tries to resolve the correct
    /// internal_id via the IIDX pointer structure before falling back to
    /// direct song_db lookup.
    fn create_chart_info_dynamic(
        &mut self,
        reader: &MemoryReader,
        song_id: u32,
        difficulty: Difficulty,
    ) -> ChartInfo {
        // Try IIDX pointer resolution: maps game_id -> correct internal_id
        if let Some(internal_id) = self
            .offsets
            .resolve_current_song_internal_id(reader, song_id)
            && internal_id != song_id
            && let Some(song) = self.game_data.song_db.get(&internal_id)
        {
            info!(
                "IIDX resolved game_id={} -> internal_id={} {:?}",
                song_id, internal_id, song.title
            );
            let mut chart = ChartInfo::from_song_info(song, difficulty, true);
            // Use game_id for ChartInfo so cross-validation with CurrentSong works
            chart.song_id = song_id;
            // Cache: insert under game_id so future lookups are instant
            let mut aliased = song.clone();
            aliased.id = song_id;
            self.game_data.song_db.insert(song_id, aliased);
            return chart;
        }

        // First check if song is already in database
        if let Some(song) = self.game_data.song_db.get(&song_id) {
            return ChartInfo::from_song_info(song, difficulty, true);
        }

        // Try to dynamically load from memory
        let entry_stride = if self.offsets.song_entry_size > 0 {
            self.offsets.song_entry_size
        } else {
            SongInfo::MEMORY_SIZE
        };
        let Some(scan_size) = entry_stride.checked_mul(5000) else {
            warn!(
                "Entry stride overflow: {} * 5000, skipping dynamic load",
                entry_stride
            );
            return ChartInfo {
                song_id,
                title: format!("Song {:05}", song_id).into(),
                title_english: format!("Song {:05}", song_id).into(),
                artist: "".into(),
                genre: "".into(),
                bpm: "".into(),
                difficulty,
                level: 0,
                total_notes: 0,
                unlocked: true,
            };
        };
        let layout = self.offsets.effective_layout();
        if let Some(song) = fetch_song_by_id(
            reader,
            self.offsets.song_db_address(),
            song_id,
            scan_size,
            entry_stride,
            &layout,
        ) {
            info!("Dynamically loaded song: {} ({})", song.title, song_id);
            let chart = ChartInfo::from_song_info(&song, difficulty, true);
            // Add to song database for future lookups
            self.game_data.song_db.insert(song_id, song);
            return chart;
        }

        // Fallback to placeholder
        debug!("Song {} not found in memory, using placeholder", song_id);
        ChartInfo {
            song_id,
            title: format!("Song {:05}", song_id).into(),
            title_english: format!("Song {:05}", song_id).into(),
            artist: "".into(),
            genre: "".into(),
            bpm: "".into(),
            difficulty,
            level: 0,
            total_notes: 0,
            unlocked: true,
        }
    }

    /// Read total_notes for the currently loaded chart from memory.
    ///
    /// The chart's total_notes is stored at CurrentSong + 0x10. This value
    /// is set by the game when a chart is loaded and remains valid through
    /// the result screen.
    fn read_current_chart_notes(&self, reader: &MemoryReader) -> u32 {
        const TOTAL_NOTES_OFFSET: u64 = 0x10;

        if self.offsets.current_song == 0 {
            return 0;
        }

        let addr = self.offsets.current_song + TOTAL_NOTES_OFFSET;
        match reader.read_i32(addr) {
            Ok(n) if (1..=10000).contains(&n) => n as u32,
            _ => 0,
        }
    }

    fn fetch_judge_data(&self, reader: &MemoryReader) -> Result<Judge> {
        let base = self.offsets.judge_data;

        let p1 = PlayerJudge {
            pgreat: reader.read_u32(base + judge::P1_PGREAT)?,
            great: reader.read_u32(base + judge::P1_GREAT)?,
            good: reader.read_u32(base + judge::P1_GOOD)?,
            bad: reader.read_u32(base + judge::P1_BAD)?,
            poor: reader.read_u32(base + judge::P1_POOR)?,
            combo_break: reader.read_u32(base + judge::P1_COMBO_BREAK)?,
            fast: reader.read_u32(base + judge::P1_FAST)?,
            slow: reader.read_u32(base + judge::P1_SLOW)?,
            measure_end: reader.read_u32(base + judge::P1_MEASURE_END)?,
        };

        let p2 = PlayerJudge {
            pgreat: reader.read_u32(base + judge::P2_PGREAT)?,
            great: reader.read_u32(base + judge::P2_GREAT)?,
            good: reader.read_u32(base + judge::P2_GOOD)?,
            bad: reader.read_u32(base + judge::P2_BAD)?,
            poor: reader.read_u32(base + judge::P2_POOR)?,
            combo_break: reader.read_u32(base + judge::P2_COMBO_BREAK)?,
            fast: reader.read_u32(base + judge::P2_FAST)?,
            slow: reader.read_u32(base + judge::P2_SLOW)?,
            measure_end: reader.read_u32(base + judge::P2_MEASURE_END)?,
        };

        Ok(Judge::from_raw_data(RawJudgeData { p1, p2 }))
    }

    fn fetch_settings(&self, reader: &MemoryReader, play_type: PlayType) -> Result<Settings> {
        let word: u64 = 4;
        let base = self.offsets.play_settings;

        let (style, assist, range, h_ran, style2) = match play_type {
            PlayType::P1 | PlayType::Dp => {
                let style = reader.read_i32(base)?;
                let assist = reader.read_i32(base + word * 2)?;
                let range = reader.read_i32(base + word * 4)?;
                let h_ran = reader.read_i32(base + word * 9)?;
                let style2 = if play_type == PlayType::Dp {
                    reader.read_i32(base + word * 5)?
                } else {
                    0
                };
                (style, assist, range, h_ran, style2)
            }
            PlayType::P2 => {
                let p2_offset = Settings::P2_OFFSET;
                let style = reader.read_i32(base + p2_offset)?;
                let assist = reader.read_i32(base + p2_offset + word * 2)?;
                let range = reader.read_i32(base + p2_offset + word * 4)?;
                let h_ran = reader.read_i32(base + p2_offset + word * 9)?;
                (style, assist, range, h_ran, 0)
            }
        };

        // Flip and battle are game-global toggles, not per-player settings.
        // Always read from the P1 base regardless of play_type.
        // (Matches Reflux C# reference: Settings.cs lines 49-50)
        let flip = reader.read_i32(base + word * 3)?;
        let battle = reader.read_i32(base + word * 8)?;

        Ok(Settings::from_raw(RawSettings {
            play_type,
            style,
            style2,
            assist,
            range,
            flip,
            battle,
            h_ran,
        }))
    }

    /// Load current unlock state from memory
    pub fn load_unlock_state(&mut self, reader: &MemoryReader) -> Result<()> {
        if self.game_data.song_db.is_empty() {
            warn!("Song database is empty, cannot load unlock state");
            return Ok(());
        }

        self.game_data.unlock_state =
            get_unlock_states(reader, self.offsets.unlock_data, &self.game_data.song_db)?;
        debug!(
            "Loaded unlock state from memory ({} entries)",
            self.game_data.unlock_state.len()
        );
        Ok(())
    }

    /// Check game version and compare with offsets version
    ///
    /// Returns (game_version, matches) where matches is true if versions match
    pub fn check_game_version(
        &self,
        reader: &MemoryReader,
        base_address: u64,
    ) -> Result<(Option<String>, bool)> {
        let game_version = find_game_version(reader, base_address)?;

        let matches = match &game_version {
            Some(version) => check_version_match(version, &self.offsets.version),
            None => false,
        };

        Ok((game_version, matches))
    }
}

#[cfg(feature = "api")]
struct LampRequest {
    endpoint: String,
    token: String,
    song_id: u32,
    title: String,
    difficulty: String,
    lamp: String,
    ex_score: u32,
    miss_count: Option<u32>,
}

#[cfg(feature = "api")]
fn send_lamp_request(req: &LampRequest) -> anyhow::Result<()> {
    let url = format!("{}/api/lamps", req.endpoint.trim_end_matches('/'));
    let mut body = serde_json::json!({
        "songId": req.song_id,
        "title": req.title,
        "difficulty": req.difficulty,
        "lamp": req.lamp,
        "exScore": req.ex_score,
    });
    if let Some(mc) = req.miss_count {
        body["missCount"] = serde_json::json!(mc);
    }

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(5)))
        .build();
    let agent: ureq::Agent = config.into();
    let response = agent
        .post(&url)
        .header("Authorization", &format!("Bearer {}", req.token))
        .send_json(&body)?;

    tracing::debug!("API response: {}", response.status());
    Ok(())
}
