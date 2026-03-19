DROP INDEX `charts_table_key_song_diff_idx`;--> statement-breakpoint
ALTER TABLE `charts` ADD `infinitas_title` text NOT NULL;--> statement-breakpoint
CREATE UNIQUE INDEX `charts_table_infttl_diff_idx` ON `charts` (`table_key`,`infinitas_title`,`difficulty`);--> statement-breakpoint
DROP INDEX `lamps_user_song_diff_idx`;--> statement-breakpoint
ALTER TABLE `lamps` ADD `title` text NOT NULL;--> statement-breakpoint
CREATE UNIQUE INDEX `lamps_user_title_diff_idx` ON `lamps` (`user_id`,`title`,`difficulty`);