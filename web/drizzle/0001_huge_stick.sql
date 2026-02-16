CREATE TABLE `rate_limits` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`key` text NOT NULL,
	`created_at` text NOT NULL
);
--> statement-breakpoint
ALTER TABLE `users` ADD `api_token_created_at` text;--> statement-breakpoint
CREATE INDEX `lamps_user_updated_at_idx` ON `lamps` (`user_id`,`updated_at`);