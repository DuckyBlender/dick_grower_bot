import matplotlib.pyplot as plt
import numpy as np
import seaborn as sns
from matplotlib.patches import Rectangle
import matplotlib.patches as mpatches

# Set up the plotting style
plt.style.use('seaborn-v0_8')
sns.set_palette("husl")

# Create figure with subplots
fig, ((ax1, ax2), (ax3, ax4)) = plt.subplots(2, 2, figsize=(15, 12))
fig.suptitle('Dick Grower Bot - Prestige System Visualization', fontsize=16, fontweight='bold')

# 1. Growth Requirement Visualization
ax1.set_title('Prestige Requirement Curve')
ax1.set_xlabel('Current Prestige Level')
ax1.set_ylabel('Required Length for Next Prestige (cm)')

# Requirement scales by +12% each prestige level
levels = np.arange(0, 16)
required_lengths = 1000 * np.power(1.12, levels)
ax1.plot(levels, required_lengths, 'g-', linewidth=2, marker='o', markersize=4)
ax1.axvline(x=0, color='r', linestyle='--', alpha=0.7, label='Starting requirement')
ax1.axvline(x=10, color='b', linestyle='--', alpha=0.7, label='Prestige level 10')
ax1.grid(True, alpha=0.3)
ax1.legend()

# Add annotations for key points
ax1.annotate('First Prestige\n(1000cm)', xy=(0, required_lengths[0]), xytext=(2, 1800),
            arrowprops=dict(arrowstyle='->', color='black'),
            fontsize=9, ha='center')
ax1.annotate(f'Level 10 target\n({required_lengths[10]:.0f}cm)', xy=(10, required_lengths[10]), xytext=(12, required_lengths[10] + 1000),
            arrowprops=dict(arrowstyle='->', color='black'),
            fontsize=9, ha='center')

# 2. Prestige Bonus Visualization
ax2.set_title('Prestige Bonus Growth')
ax2.set_xlabel('Prestige Level')
ax2.set_ylabel('Bonus Growth Per /grow (cm)')

# Diminishing returns: rounded sqrt(level)
prestige_lvls = np.arange(0, 31)
bonuses = np.round(np.sqrt(prestige_lvls), 0)
ax2.bar(prestige_lvls, bonuses, color='green', alpha=0.7, edgecolor='black')
ax2.plot(prestige_lvls, bonuses, 'ro-', linewidth=2, markersize=6)
ax2.grid(True, alpha=0.3)

# Add value labels on bars
for i, (lvl, bonus) in enumerate(zip(prestige_lvls, bonuses)):
    ax2.text(lvl, bonus + 0.05, f'{bonus}cm', ha='center', va='bottom', fontsize=8)

# 3. Prestige Points Gain Visualization
ax3.set_title('Prestige Points Gain')
ax3.set_xlabel('Length Before Prestige (cm)')
ax3.set_ylabel('Points Gained (10% of length)')

# Calculate points gained (10% of length)
lengths_for_points = np.array([1000, 1500, 2000, 3000, 4000, 5000, 7000, 9000])
points_gained = lengths_for_points * 0.1
bars = ax3.bar(range(len(lengths_for_points)), points_gained, 
               color=['red', 'orange', 'yellow', 'green', 'blue', 'purple', 'pink', 'brown'],
               edgecolor='black', alpha=0.7)
ax3.set_xticks(range(len(lengths_for_points)))
ax3.set_xticklabels([f'{l}' for l in lengths_for_points], rotation=45)
ax3.grid(True, alpha=0.3)

# Add value labels on bars
for i, (bar, points) in enumerate(zip(bars, points_gained)):
    ax3.text(bar.get_x() + bar.get_width()/2, bar.get_height() + 10, 
             f'{int(points)} pts', ha='center', va='bottom', fontsize=8)

# 4. Prestige Progression Flowchart
ax4.set_title('Prestige Process Flow')
ax4.set_xlim(0, 10)
ax4.set_ylim(0, 10)
ax4.axis('off')

# Draw flowchart elements
steps = [
    "1. Grow dick to required length",
    "2. Use /prestige command",
    "3. Length resets to 0 cm",
    "4. Gain prestige points",
    "5. Unlock permanent growth bonus",
    "6. Next prestige requires more length"
]

# Position steps in a vertical flow
for i, step in enumerate(steps):
    y_pos = 9 - i * 1.3
    ax4.add_patch(Rectangle((2, y_pos-0.3), 6, 0.8, facecolor='lightblue', edgecolor='black'))
    ax4.text(5, y_pos, step, ha='center', va='center', fontsize=10, fontweight='bold')

# Add arrows between steps
for i in range(len(steps)-1):
    y1 = 9 - i * 1.3 - 0.3
    y2 = 9 - (i+1) * 1.3 + 0.5
    ax4.annotate('', xy=(5, y2), xytext=(5, y1),
                arrowprops=dict(arrowstyle='->', color='black', lw=1.5))

# Add explanatory text
explanation = ("Each prestige level provides:\n"
               "• Bonus growth from sqrt(prestige level)\n"
               "• Permanent prestige points\n"
               "• Higher status in the community\n\n"
               "Balanced progression system:\n"
               "• Requirement scales by +12% each level\n"
               "• Rewards long-term players\n"
               "• Prevents runaway economy inflation")

ax4.text(0.5, 1, explanation, fontsize=9, va='bottom',
         bbox=dict(boxstyle="round,pad=0.3", facecolor="wheat", alpha=0.7))

# Adjust layout and save
plt.tight_layout()
output_path = 'prestige_system_visualization.png'
plt.savefig(output_path, dpi=300, bbox_inches='tight')
print(f"Prestige system visualization saved to: {output_path}")

# Show the plot
plt.show()

print("\n" + "="*50)
print("DICK GROWER BOT - PRESTIGE SYSTEM")
print("="*50)
print("Key Features:")
print("• Requirement: 1000cm base, +12% each prestige level")
print("• Bonus: rounded sqrt(prestige level)")
print("• Points: 10% of length at prestige")
print("• Reset: Length returns to 0cm")
print("• Progression: Scales with increasing requirements")
print("\nBalance Considerations:")
print("• Diminishing growth bonus curve")
print("• Increasing requirement curve")
print("• Significant time investment")
print("• Rewards commitment, not pay-to-win")
print("="*50)
