//! The command line: one subcommand and at most one argument.

const USAGE: &str = "\
usage: crown-android <command>
  launch <package>   open an Android app
  stop               stop Android now
  status             show whether Android runs
  diagnose           write a diagnostics archive and print its path
  install <apk>      install an APK file
  open <uri>         open a market: or https: link in Android
  settings <package> show an app's Android settings";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Launch { package: String },
    Stop,
    Status,
    Diagnose,
    Install { apk: String },
    Open { uri: String },
    Settings { package: String },
}

/// The command in `args`, or the usage text.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, &'static str> {
    let mut args = args.into_iter();
    let name = args.next().ok_or(USAGE)?;
    let mut operand = || {
        args.next()
            .filter(|operand| !operand.is_empty())
            .ok_or(USAGE)
    };
    let command = match name.as_str() {
        "launch" => Command::Launch {
            package: operand()?,
        },
        "stop" => Command::Stop,
        "status" => Command::Status,
        "diagnose" => Command::Diagnose,
        "install" => Command::Install { apk: operand()? },
        "open" => Command::Open { uri: operand()? },
        "settings" => Command::Settings {
            package: operand()?,
        },
        _ => return Err(USAGE),
    };
    match args.next() {
        Some(_) => Err(USAGE),
        None => Ok(command),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_line(line: &str) -> Result<Command, &'static str> {
        parse(line.split_whitespace().map(str::to_owned))
    }

    #[test]
    fn parses_every_command() {
        assert_eq!(
            parse_line("launch com.whatsapp"),
            Ok(Command::Launch {
                package: "com.whatsapp".into()
            })
        );
        assert_eq!(parse_line("stop"), Ok(Command::Stop));
        assert_eq!(parse_line("status"), Ok(Command::Status));
        assert_eq!(parse_line("diagnose"), Ok(Command::Diagnose));
        assert_eq!(
            parse_line("install /tmp/a.apk"),
            Ok(Command::Install {
                apk: "/tmp/a.apk".into()
            })
        );
        assert_eq!(
            parse_line("open market://details?id=x"),
            Ok(Command::Open {
                uri: "market://details?id=x".into()
            })
        );
        assert_eq!(
            parse_line("settings com.whatsapp"),
            Ok(Command::Settings {
                package: "com.whatsapp".into()
            })
        );
    }

    #[test]
    fn rejects_missing_or_extra_arguments() {
        assert_eq!(parse_line("launch"), Err(USAGE));
        assert_eq!(parse_line("stop now"), Err(USAGE));
        assert_eq!(parse_line(""), Err(USAGE));
        assert_eq!(parse_line("reboot"), Err(USAGE));
    }
}
