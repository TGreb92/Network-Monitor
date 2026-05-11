//! # Help Tab - Explains all metrics, diagnosis logic, and UI elements
//!
//! Provides in-app documentation so users understand what each
//! stat card, chart element, and diagnosis verdict means.

use eframe::egui;

/// Render the Help tab contents
pub fn render(ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        render_sidebar_section(ui);
        ui.add_space(12.0);
        render_metrics_section(ui);
        ui.add_space(12.0);
        render_chart_section(ui);
        ui.add_space(12.0);
        render_diagnosis_section(ui);
        ui.add_space(12.0);
        render_config_section(ui);
        ui.add_space(12.0);
        render_tips_section(ui);
    });
}

fn render_sidebar_section(ui: &mut egui::Ui) {
    ui.heading("📋 Sidebar - Stats Explained");
    ui.add_space(4.0);

    help_item(ui, "Target Selector",
        "Dropdown to choose which preset target to ping.\n\
         Presets are configured in the Config tab. Changing target takes effect on next Start.");

    help_item(ui, "Sent / Received",
        "Total number of pings sent and successful replies received.\n\
         The difference is the number of lost packets.");

    help_item(ui, "Loss %",
        "Percentage of pings that timed out: (sent - received) / sent × 100");

    help_item(ui, "Lost",
        "Absolute count of individual pings that got no reply.");

    help_item(ui, "Loss Events",
        "Number of distinct connectivity drops (loss batches).\n\
         A loss event is a cluster of consecutive timeouts.\n\
         Example: ✅✅❌❌❌✅✅❌✅ = 7 lost but only 2 loss events.\n\
         This tells you how many TIMES connectivity dropped, not just how many packets were lost.\n\
         Fewer events with many lost packets = sustained outage.\n\
         Many events with few lost packets each = intermittent flapping.");

    help_item(ui, "Elapsed / Progress",
        "Shows how long the current test has been running.\n\
         If a duration is set in Config, a progress bar shows remaining time.");

    help_item(ui, "Export CSV / JSON",
        "Save all ping results and interval reports to a file.\n\
         Files are saved next to the executable with a timestamp in the filename.");
}

fn render_metrics_section(ui: &mut egui::Ui) {
    ui.heading("📊 Monitor Tab - Metrics Explained");
    ui.add_space(4.0);

    help_item(ui, "Packet Loss",
        "Percentage of pings that got no reply.\n\
         Formula: (sent - received) / sent × 100\n\
         🟢 < 1% = Excellent  🟡 1-5% = Acceptable  🔴 > 5% = Problem");

    help_item(ui, "Avg Latency",
        "Average round-trip time across all successful pings.\n\
         Lower is better. Measures how fast your packets reach the target and return.\n\
         🟢 < 30ms = Great  🟡 30-100ms = OK  🔴 > 100ms = Slow");

    help_item(ui, "Min / Max Latency",
        "The fastest and slowest successful ping recorded.\n\
         A big gap between min and max suggests unstable connection (jitter).");

    help_item(ui, "Jitter",
        "Average variation between consecutive pings.\n\
         Formula: mean of |latency[n] - latency[n-1]|\n\
         High jitter means inconsistent connection - bad for gaming, VoIP, video calls.\n\
         🟢 < 5ms = Stable  🟡 5-20ms = Noticeable  🔴 > 20ms = Unstable");

    help_item(ui, "Connection Verdict",
        "Overall quality rating based on packet loss AND average latency:\n\
         • Excellent - loss < 1% and avg < 50ms\n\
         • Good - loss < 5% and avg < 100ms\n\
         • Fair - loss < 15%\n\
         • Poor - loss >= 15%");
}

fn render_chart_section(ui: &mut egui::Ui) {
    ui.heading("📈 Latency Chart");
    ui.add_space(4.0);

    help_item(ui, "Blue Line (Latency)",
        "Shows the round-trip time of each successful ping over time.\n\
         X-axis is elapsed time (minutes:seconds). Y-axis is latency in milliseconds.\n\
         Spikes indicate momentary slowdowns. Gaps indicate timeouts.");

    help_item(ui, "Red Dots (Timeouts)",
        "Each red dot at the bottom of the chart marks a failed ping.\n\
         Clusters of red dots show periods of packet loss.\n\
         Isolated dots may just be transient network blips.");

    help_item(ui, "Orange Line (Gateway)",
        "Only shown when gateway monitoring is enabled.\n\
         Shows latency to your router/gateway.\n\
         Compare with the blue line to see if delays are local or external.");

    help_item(ui, "Hover Tooltips",
        "Hover over any point on the chart to see:\n\
         • Elapsed time (m:ss)\n\
         • Latency value in ms, or timeout indicator");
}

fn render_diagnosis_section(ui: &mut egui::Ui) {
    ui.heading("🏥 Gateway Diagnosis - How It Works");
    ui.add_space(4.0);

    ui.label("When gateway monitoring is enabled, the app pings both your router \
              AND the external target simultaneously. By comparing the two, it can \
              pinpoint where problems originate:");
    ui.add_space(8.0);

    help_item(ui, "✅ All Clear",
        "Both gateway and external loss are below 2%.\n\
         Your local network and internet connection are healthy.");

    help_item(ui, "⚠ Local Network Issue",
        "Gateway loss is above 2%.\n\
         Packets are being lost between your computer and your router.\n\
         Causes: Wi-Fi interference, bad Ethernet cable, router overload.\n\
         Fix: Move closer to router, switch to wired, restart router.");

    help_item(ui, "ISP / Route Issue",
        "Gateway loss is fine (< 2%) but external loss is above 2%.\n\
         Your local network is healthy - the problem is between your router and the target.\n\
         Causes: ISP congestion, routing problems, target server issues.\n\
         Fix: Contact ISP, try a different target, test at different times.");

    help_item(ui, "Collecting...",
        "Not enough data yet to make a diagnosis.\n\
         Wait for at least a few pings to both targets.");
}

fn render_config_section(ui: &mut egui::Ui) {
    ui.heading("⚙ Config Settings");
    ui.add_space(4.0);

    help_item(ui, "Target Presets",
        "Add, edit, and delete named target presets.\n\
         Each preset has a name (e.g. \"Game Server EU\") and a host (IP or hostname).\n\
         Select a preset in the sidebar dropdown to use it.\n\
         Presets are saved to network-monitor.toml.");

    help_item(ui, "Timeout (ms)",
        "How long to wait for a reply before marking it as lost.\n\
         Default: 2000ms. Increase for high-latency connections (satellite, VPN).");

    help_item(ui, "Report Interval (s)",
        "How often to generate summary statistics in the Interval Reports table.\n\
         Default: 60s. Use shorter intervals for more granular data.");

    help_item(ui, "Ping Frequency (ms)",
        "Time between consecutive pings. Default: 1000ms (1 ping/sec).\n\
         Lower = more data but more network traffic. Minimum: 100ms.");

    help_item(ui, "Gateway Monitoring",
        "When enabled, also pings your default gateway (router) in parallel.\n\
         This allows the Diagnosis feature to distinguish local vs external issues.");

    help_item(ui, "Auto-detect Gateway",
        "Automatically finds your router IP via 'ipconfig' on startup.\n\
         Only works on Windows. You can also click 🔍 Detect in the sidebar.");

    help_item(ui, "Test Duration",
        "Optional fixed-duration test. Set to 0 for unlimited (run until stopped).\n\
         When set, the sidebar shows a progress bar with remaining time.\n\
         The test auto-stops and flushes a final interval report when time is up.");
}

fn render_tips_section(ui: &mut egui::Ui) {
    ui.heading("💡 Tips");
    ui.add_space(4.0);

    ui.label("• Run for at least 5 minutes to get meaningful data");
    ui.label("• Enable gateway monitoring to diagnose local vs ISP issues");
    ui.label("• Use the Console tab to spot individual timeout events");
    ui.label("• Export to CSV/JSON to save results for later analysis");
    ui.label("• Try different targets (Cloudflare 1.1.1.1, Quad9 9.9.9.9) to isolate route-specific problems");
    ui.label("• Test at different times of day to find congestion patterns");
}

/// Render a single help item with a bold title and description
fn help_item(ui: &mut egui::Ui, title: &str, description: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("{}:", title));
    });
    ui.label(description);
    ui.add_space(6.0);
}
