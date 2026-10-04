# 🍆 Dick Grower Bot

A Discord bot where users compete to grow the biggest virtual dick in their server. Built in Rust with [Serenity](https://github.com/serenity-rs/serenity) and [SQLx](https://github.com/launchbadge/sqlx) on SQLite.

## Commands

All commands work in servers only. Each server has its own leaderboard and stats.

| Command | Description | Cooldown |
|---------|-------------|----------|
| `/grow` | Grow 1-10 cm, plus any active boosts | 60 minutes |
| `/daily` | Claim a random perk | Once per UTC day |
| `/viagra` | +20% growth for 6 hours | 20 hours |
| `/event` | Show the current global event, or when the next one may start | None |
| `/pvp <bet>` | Open a dick battle anyone can accept | None |
| `/gift <user> <amount>` | Give some of your length to someone else | None |
| `/dickoftheday` | Award 10-25 cm to a random active grower | Once per server per UTC day |
| `/stats [user]` | Length, rank, streaks, perks, viagra and battle stats | None |
| `/top` | Server top 10, plus your own position | None |
| `/global` | Global top 10 across all servers | None |
| `/help` | Command overview | None |

## Game Mechanics

### Growing
- `/grow` adds 1-10 cm and is always positive.
- Percentage boosts (viagra, daily boost, Growth Surge event) are added together and applied to the roll.
- The reply shows your new length, server rank, every boost that applied and when you can grow next.

### Daily Perks
`/daily` gives one random perk, each equally likely:
- **Bonus cm**: 5-15 cm straight away
- **Growth boost**: your next `/grow` gets +50%
- **Cooldown skip**: your next `/grow` while on cooldown ignores the cooldown
- **Streak saver**: one missed UTC day doesn't break your growth streak
- **Lucky roll**: your next `/grow` rolls twice and keeps the better result

Unused perks are listed under 🎒 Perks in `/stats`.

### Growth Streaks
- Your first `/grow` of each UTC day continues your streak; missing a day resets it to 1 (unless you have a streak saver).
- Each streak day also gives bonus cm, rising slowly from 1 cm up to a maximum of 5 cm.

### Global Events
Every 4 hours, on the UTC boundary (00:00, 04:00, ...), there is a 50% chance that an event starts. It lasts until the next boundary and applies to every server.

| Event | Effect |
|-------|--------|
| Growth Surge | +25% growth |
| Fast Hands | `/grow` cooldown drops to 30 minutes |
| Extended Pharmacy Hours | New `/viagra` doses last 12 hours |
| Double Trouble | Every `/grow` rolls twice and keeps the better result |
| Quick Sprouts | `/grow` gives 1-5 cm but has a 15 minute cooldown |
| Jackpot Window | Each `/grow` has a 1 in 10 chance of +25 cm |
| Community Pump | Each `/grow` adds 1 cm to a global pot. When the event ends, the pot goes to one random person who grew during it |

The bot's status shows the active event.

### Dick Battles
- `/pvp <bet>` posts a challenge with **Accept** and **Cancel** buttons. It expires after 24 hours.
- Both players roll 1-100. The higher roll takes the bet; a tie returns everything.
- Both players must be able to cover the bet when the battle resolves.
- You can only have one open challenge. Starting a new one cancels the old one.

### Dick of the Day
- Anyone can run `/dickoftheday` once per server per UTC day.
- It picks a random user who has grown in the last 7 days. At least 2 such users are needed.

## Setup

### Requirements
- Rust 1.94+
- A Discord bot token. The bot only needs the `GUILDS` intent.
- [`sqlx-cli`](https://crates.io/crates/sqlx-cli) to create the database: `cargo install sqlx-cli --no-default-features --features sqlite`

### Running Locally
```bash
cp .env.example .env               # then fill in DISCORD_TOKEN
sqlx database create
sqlx migrate run
cargo run --release
```

On startup the bot adds any columns that are missing from older databases. It does not create tables, so run `sqlx migrate run` for a new database.

### Docker Deployment
`deploy.sh` pulls the latest code and builds the image. If the build succeeds, it stops the old container, backs up `database.sqlite` (keeping the 10 newest backups) and starts the new container. `.env` is passed at runtime and is never copied into the image.

```bash
./deploy.sh
```

## Development

### Database Queries
Queries are checked at compile time with `sqlx::query!`. The `.sqlx/` directory stores query metadata so the project builds without a database (`SQLX_OFFLINE=true`, which the Dockerfile sets). After changing a query, regenerate it:

```bash
export DATABASE_URL=sqlite:dev.sqlite
sqlx database create && sqlx migrate run
cargo sqlx prepare -- --all-targets
```

### Checks
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### Project Layout
```
src/
├── main.rs          # Startup, interaction routing, background scheduler
├── db.rs            # Schema upgrades and shared queries (users, ranks, history)
├── time.rs          # Timestamp parsing/formatting and Discord timestamps
├── utils.rs         # Embed helpers, colors and text formatting
└── commands/        # One module per slash command (register + run)
migrations/          # SQLx migrations
```

### Database Schema
- `dicks`: one row per user per server, holding length, stats, perks, streaks and viagra state
- `length_history`: every length change, with `growth_type` set to `grow`, `streak`, `daily_bonus`, `dotd`, `gift_sent`, `gift_received`, `pvp_won`, `pvp_lost` or `community_pot`
- `guild_settings`: per-server state (last Dick of the Day)
- `global_events`: past and current global events, including community pot payouts

## Community

Join our Discord for updates: [Discord Server](https://discord.gg/39nqUzYGbe)

---

*Remember: It's not about the size, it's about... actually, it is about the size.* 🍆
