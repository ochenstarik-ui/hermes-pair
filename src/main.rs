use clap::Parser;
use eframe::egui::Vec2;
use hermes_pair::app::HermesPairApp;
use hermes_pair::cli::{run_once, run_terminal_loop, CliArgs, CliCommand};
use hermes_pair::config::load_or_create_config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();
    let config = load_or_create_config()?;

    if let Some(CliCommand::Qr(qr_args)) = &args.command {
        let port = qr_args.port.unwrap_or(args.port);
        let iface = qr_args.interface.as_deref().or(args.interface.as_deref());
        let ttl = qr_args.ttl.unwrap_or(args.ttl);
        return run_once(&config, port, iface, ttl).await;
    }

    if args.no_gui {
        return run_once(&config, args.port, args.interface.as_deref(), args.ttl).await;
    }

    if args.terminal {
        return run_terminal_loop(&config, args.port, args.interface.as_deref(), args.ttl).await;
    }

    // Launch GUI
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(420.0, 620.0))
            .with_min_inner_size(Vec2::new(380.0, 520.0))
            .with_title("Hermes Pair"),
        ..Default::default()
    };

    let config_clone = config.clone();
    let port = args.port;
    let iface = args.interface.clone();
    let ttl = args.ttl;

    let res = eframe::run_native(
        "Hermes Pair",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(HermesPairApp::new(
                cc,
                config_clone,
                port,
                iface,
                ttl,
            )))
        }),
    );

    if let Err(e) = res {
        eprintln!("Failed to launch GUI: {}", e);
        eprintln!("Falling back to terminal mode...");
        return run_terminal_loop(&config, args.port, args.interface.as_deref(), args.ttl).await;
    }

    Ok(())
}
