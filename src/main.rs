use clap::Parser;
use eframe::egui::Vec2;
use hermes_pair::app::HermesPairApp;
use hermes_pair::cli::{resolve_cli_endpoint, run_once, run_terminal_loop, CliArgs, CliCommand};
use hermes_pair::config::load_or_create_config;
use hermes_pair::pairing::validate_ttl;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();
    let config = load_or_create_config()?;

    if let Some(CliCommand::Qr(ref qr_args)) = args.command {
        let hermes_url = qr_args.hermes_url.as_deref().or(args.hermes_url.as_deref());
        let (scheme, port) = resolve_cli_endpoint(hermes_url, qr_args.port.or(args.port))?;
        let iface = qr_args.interface.as_deref().or(args.interface.as_deref());
        let ttl = qr_args.ttl.unwrap_or(args.ttl);
        validate_ttl(ttl)?;
        return run_once(&config, hermes_url, &scheme, port, iface, ttl).await;
    }

    let hermes_url_str = args.hermes_url.as_deref();
    let (scheme, port) = resolve_cli_endpoint(hermes_url_str, args.port)?;
    let ttl = args.ttl;
    validate_ttl(ttl)?;

    if args.no_gui {
        return run_once(
            &config,
            hermes_url_str,
            &scheme,
            port,
            args.interface.as_deref(),
            ttl,
        )
        .await;
    }

    if args.terminal {
        return run_terminal_loop(
            &config,
            hermes_url_str,
            &scheme,
            port,
            args.interface.as_deref(),
            ttl,
        )
        .await;
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
    let hermes_url_owned = args.hermes_url.clone();
    let scheme_clone = scheme.clone();
    let iface = args.interface.clone();

    let res = eframe::run_native(
        "Hermes Pair",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(HermesPairApp::new(
                cc,
                config_clone,
                hermes_url_owned,
                scheme_clone,
                port,
                iface,
                ttl,
            )))
        }),
    );

    if let Err(e) = res {
        eprintln!("Failed to launch GUI: {}", e);
        eprintln!("Falling back to terminal mode...");
        return run_terminal_loop(
            &config,
            args.hermes_url.as_deref(),
            &scheme,
            port,
            args.interface.as_deref(),
            ttl,
        )
        .await;
    }

    Ok(())
}
