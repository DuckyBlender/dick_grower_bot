# 🪴 Plant Grower Bot

A Discord bot where users compete to grow the biggest virtual plant in their server! Built with Rust using Serenity and SQLx.
This bot was originally called "Dick Grower Bot", but has been re-themed to be more appropriate for all audiences.

Servers can choose to use the original "Dick Theme" through the settings command if they prefer.

## Features

### Core Commands
- **`/grow`** - Grow your plant once per hour (always positive growth!)
- **`/top`** - View the server's plant leaderboard
- **`/global`** - View the global plant leaderboard across all servers
- **`/stats <user>`** - View detailed plant statistics
- **`/help`** - Show command help

### Progression System
- **`/prestige`** - Reset your plant to gain permanent bonuses
- Strategic long-term growth planning
- Prestige points for status and progression

### Battle System
- **`/pvp <bet>`** - Challenge others to plant battles with cm bets
- Interactive button-based acceptance system
- Win streaks and battle statistics tracking
- Risk vs reward betting mechanics

### Social Features
- **`/gift <user> <amount>`** - Gift some of your length to another user
- **`/plantoftheday`** - Random daily Plant of the Day selection with bonuses
- Length sharing and generosity mechanics

### Enhancement System
- **`/viagra`** - Boost your growth by 20% for 6 hours (3-day cooldown)
- Temporary performance enhancement
- Strategic timing for maximum gains

## Technical Features

### Database Design
- **Optimized queries** with proper indexing
- **Length history tracking** - Logs all growth events over time
- **Guild-specific data** - Each server has its own leaderboards
- **Comprehensive statistics** - PVP records, growth counts, etc.

### Performance Optimizations
- **Guild name caching** - Server names cached for 12 hours to reduce API calls
- **Single-query leaderboards** - Optimized `/top` command performance
- **Efficient ranking** calculations

### Data Integrity
- **Database transactions** for gift transfers
- **User validation** and error handling
- **Automatic user creation** for new participants

## Commands Reference

| Command | Description | Cooldown |
|---------|-------------|----------|
| `/grow` | Grow your plant (1-10cm, +20% with viagra) | 60 minutes |
| `/top` | Server leaderboard | None |
| `/global` | Global leaderboard | None |
| `/pvp <bet>` | Plant battle with cm betting | None |
| `/stats <user>` | View user statistics | None |
| `/plantoftheday` | Random daily winner selection | 24 hours |
| `/gift <user> <amount>` | Transfer length to another user | None |
| `/prestige` | Reset plant for permanent bonuses | None |
| `/settings` | Configure bot settings | None |
| `/viagra` | 20% growth boost for 6 hours | 72 hours |
| `/help` | Show command help | None |

## Growth Mechanics

### Base Growth
- **Range**: 1-10 cm per growth
- **Frequency**: Once per hour
- **Always positive** - No more shrinkage!

### Viagra Enhancement
- **Boost**: +20% to base growth
- **Duration**: 6 hours
- **Cooldown**: 20 hours
- **Visual indicator** in growth messages

### Special Events
- **Plant of the Day**: 10-25 cm bonus (daily)
- **PVP Victories**: Win opponent's bet amount
- **Gifts**: Receive length from generous users

## Database Schema

### Main Tables
- `plants` - User data, lengths, stats, viagra status
- `length_history` - Growth tracking over time
- `guild_settings` - Server-specific settings

### Growth Types Tracked
- `grow` - Regular hourly growth
- `gift_sent` / `gift_received` - Length transfers
- `pvp_won` / `pvp_lost` - Battle results  
- `dotd` - Plant of the Day bonuses

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
Located in `migrations/` directory:
- `20250309235354_initialize.sql` - Core tables
- `20250310000000_add_features.sql` - New features (viagra, history, caching)

## Features Implemented

### Recent Updates
✅ **Gift System** - Users can transfer length to others  
✅ **Viagra Enhancement** - Temporary growth boosts  
✅ **Positive-Only Growth** - Removed negative growth mechanics  
✅ **Length History Logging** - Track all growth events  
✅ **Optimized Leaderboards** - Better query performance  
✅ **Guild Name Caching** - Reduced API calls for global leaderboard  

### Battle System
✅ **PVP Challenges** with betting  
✅ **Win Streak Tracking**  
✅ **Interactive Buttons**  
✅ **Risk/Reward Mechanics**  

### Statistics & Tracking
✅ **Comprehensive User Stats**  
✅ **Server Rankings**  
✅ **Global Leaderboards**  
✅ **Growth History**  

## Bot Invite

[Add to your server](YOUR_BOT_INVITE_LINK_HERE)

## Community

Join our Discord for updates: [Discord Server](https://discord.gg/39nqUzYGbe)

---

*Remember: It's not about the size, it's about... actually, it is about the size.* 🍆
