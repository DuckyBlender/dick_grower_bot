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
fig.suptitle('Plant Grower Bot - Prestige System Visualization', fontsize=16, fontweight='bold')

# 1. Growth Requirement Visualization
ax1.set_title('Prestige Requirements')
ax1.set_xlabel('Plant Length (cm)')
ax1.set_ylabel('Prestige Level')

# Show the requirement curve (1000cm per prestige)
lengths = np.arange(0, 5001, 100)
prestige_levels = lengths // 1000
ax1.plot(lengths, prestige_levels, 'g-', linewidth=2, marker='o', markersize=4)
ax1.axhline(y=0, color='r', linestyle='--', alpha=0.7, label='No Prestige')
ax1.axhline(y=1, color='orange', linestyle='--', alpha=0.7, label='Prestige 1')
ax1.axhline(y=5, color='purple', linestyle='--', alpha=0.7, label='Prestige 5')
ax1.axhline(y=10, color='b', linestyle='--', alpha=0.7, label='Prestige 10')
ax1.fill_between(lengths, 0, 1, alpha=0.1, color='red')
ax1.fill_between(lengths, 1, 2, alpha=0.1, color='orange')
ax1.fill_between(lengths, 2, 10, alpha=0.1, color='yellow')
ax1.set_xlim(0, 5000)
ax1.set_ylim(0, 12)
ax1.grid(True, alpha=0.3)
ax1.legend()

# Add annotations for key points
ax1.annotate('First Prestige\n(1000cm)', xy=(1000, 1), xytext=(1200, 2),
            arrowprops=dict(arrowstyle='->', color='black'),
            fontsize=9, ha='center')
ax1.annotate('Legendary\n(5000cm)', xy=(5000, 5), xytext=(4500, 7),
            arrowprops=dict(arrowstyle='->', color='black'),
            fontsize=9, ha='center')

# 2. Prestige Bonus Visualization
ax2.set_title('Prestige Bonuses')
ax2.set_xlabel('Prestige Level')
ax2.set_ylabel('Bonus Growth (cm per /grow)')

# Calculate bonuses (0.5cm per prestige level)
prestige_lvls = np.arange(0, 11)
bonuses = prestige_lvls * 0.5
ax2.bar(prestige_lvls, bonuses, color='green', alpha=0.7, edgecolor='black')
ax2.plot(prestige_lvls, bonuses, 'ro-', linewidth=2, markersize=6)
ax2.grid(True, alpha=0.3)

# Add value labels on bars
for i, (lvl, bonus) in enumerate(zip(prestige_lvls, bonuses)):
    ax2.text(lvl, bonus + 0.05, f'{bonus}cm', ha='center', va='bottom', fontsize=8)

# 3. Prestige Points Gain Visualization
ax3.set_title('Prestige Points Gain')
ax3.set_xlabel('Plant Length Before Prestige (cm)')
ax3.set_ylabel('Points Gained (10% of length)')

# Calculate points gained (10% of length)
lengths_for_points = np.arange(1000, 5001, 500)
points_gained = lengths_for_points * 0.1
bars = ax3.bar(range(len(lengths_for_points)), points_gained, 
               color=['red', 'orange', 'yellow', 'green', 'blue', 'purple', 'pink', 'brown', 'gray', 'olive'],
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
    "1. Grow plant to 1000+ cm",
    "2. Use /prestige command",
    "3. Plant resets to 0 cm",
    "4. Gain prestige points",
    "5. Unlock growth bonus",
    "6. Repeat for higher levels"
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
               "• 0.5cm bonus growth per /grow\n"
               "• Permanent prestige points\n"
               "• Higher status in the community\n\n"
               "Balanced progression system:\n"
               "• Requires significant commitment\n"
               "• Rewards long-term players\n"
               "• Maintains game balance")

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
print("PLANT GROWER BOT - PRESTIGE SYSTEM")
print("="*50)
print("Key Features:")
print("• Requirement: 1000cm to prestige")
print("• Bonus: 0.5cm per prestige level")
print("• Points: 10% of length at prestige")
print("• Reset: Length returns to 0cm")
print("• Progression: Unlimited prestige levels")
print("\nBalance Considerations:")
print("• High barrier to entry (1000cm)")
print("• Diminishing returns per level")
print("• Significant time investment required")
print("• Rewards commitment, not pay-to-win")
print("="*50)
