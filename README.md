# 🍆 Dick Grower Bot

A Discord bot where users compete to grow the biggest virtual dick in their server. Built with Rust using Serenity and SQLx.

## Features

### Core Commands
- **`/grow`** - Grow your dick once per hour (always positive growth)
- **`/top`** - View the server dick leaderboard
- **`/global`** - View the global dick leaderboard across all servers
- **`/stats <user>`** - View detailed dick stats
- **`/help`** - Show command help

### Prestige System
- **`/prestige`** - Reset to 0 cm for permanent progression bonuses
- Prestige requirements scale each level to keep economy healthy
- Prestige growth bonus uses diminishing returns for balance
- Prestige points track long-term status

### Battle System
- **`/pvp <bet>`** - Challenge others to dick battles with cm bets
- Interactive button acceptance flow
- Win streaks and battle tracking
- Risk/reward betting

### Social Features
- **`/gift <user> <amount>`** - Gift your length to another user
- **`/dickoftheday`** - Random daily Dick of the Day bonus
- Length sharing and community competition

### Enhancement System
- **`/viagra`** - Boost growth by 20% for 6 hours
- Strategic timing for max gains

## Commands Reference

| Command | Description | Cooldown |
|---------|-------------|----------|
| `/grow` | Grow your dick (1-10cm, +20% with viagra, +prestige bonus) | 60 minutes |
| `/top` | Server leaderboard | None |
| `/global` | Global leaderboard | None |
| `/pvp <bet>` | Dick battle with cm betting | None |
| `/stats <user>` | View user statistics | None |
| `/dickoftheday` | Random daily winner selection | 24 hours |
| `/gift <user> <amount>` | Transfer length to another user | None |
| `/prestige` | Reset for permanent bonuses | None |
| `/viagra` | 20% growth boost for 6 hours | 72 hours |
| `/help` | Show command help | None |

## Database Schema

### Main Tables
- `dicks` - User data, lengths, stats, viagra and prestige info
- `length_history` - Growth tracking over time
- `prestige_history` - Prestige reset history
- `guild_settings` - Server-specific settings

## Development

### Prerequisites
- Rust 1.70+
- SQLite 3
- Discord Bot Token

### Setup
1. Clone the repository
2. Copy `.env.example` to `.env` and configure
3. Run `cargo run` to start the bot
4. Migrations run automatically on startup

### Database Migrations
- `20250309235354_initialize.sql` - Core tables
- `20250310000000_add_features.sql` - Viagra, history, caching
- `20250310000001_add_prestige.sql` - Prestige progression

## Community

Join our Discord for updates: [Discord Server](https://discord.gg/39nqUzYGbe)

---

*Remember: it's not about the size, it's about... actually, it is about the size.* 🍆
