use super::*;

const VECTORS: &str = include_str!("../../../../contracts/agent-wire-vectors.txt");

/// One sample of every record, named as in the vectors file.
fn samples() -> Vec<(&'static str, Record)> {
    vec![
        (
            "hello",
            Record::Hello {
                wire: 1,
                bundle: "2026.11.0".into(),
            },
        ),
        (
            "set_size",
            Record::SetSize {
                task: 7,
                width: 1280,
                height: 800,
                serial: 42,
            },
        ),
        ("focus", Record::Focus { task: 7 }),
        ("close", Record::Close { task: 7 }),
        ("back", Record::Back { task: 7 }),
        (
            "visibility",
            Record::Visibility {
                task: 7,
                visible: false,
            },
        ),
        (
            "task_info",
            Record::TaskInfo {
                task: 7,
                package: "com.whatsapp".into(),
                label: "WhatsApp".into(),
                color: 0xFF07_5E54,
                resizable: true,
                min_width: 320,
                min_height: 480,
            },
        ),
        (
            "slot_applied",
            Record::SlotApplied {
                task: 7,
                serial: 42,
                slot: Rect {
                    left: 0,
                    top: 0,
                    right: 1280,
                    bottom: 800,
                },
            },
        ),
        ("task_removed", Record::TaskRemoved { task: 7 }),
        (
            "activate",
            Record::Activate {
                task: 7,
                token: "tok-1".into(),
            },
        ),
        (
            "launch",
            Record::Launch {
                package: "com.whatsapp".into(),
                width: 1280,
                height: 800,
                token: String::new(),
            },
        ),
        ("list_tasks", Record::ListTasks),
        ("query_keep_alive", Record::QueryKeepAlive),
        ("query_gsf_id", Record::QueryGsfId),
        ("shutdown", Record::Shutdown),
        (
            "open_uri",
            Record::OpenUri {
                uri: "market://details?id=com.whatsapp".into(),
            },
        ),
        (
            "install_package",
            Record::InstallPackage {
                path: "app.apk".into(),
            },
        ),
        ("ready", Record::Ready { boot_ms: 9_500 }),
        (
            "task_opened",
            Record::TaskOpened {
                task: 7,
                package: "com.whatsapp".into(),
            },
        ),
        ("task_closed", Record::TaskClosed { task: 7 }),
        ("tasks", Record::Tasks { tasks: vec![7, 9] }),
        (
            "keep_alive",
            Record::KeepAlive {
                reasons: KeepAliveReasons(KeepAliveReasons::AUDIO | KeepAliveReasons::DOWNLOAD),
            },
        ),
        ("gsf_id", Record::GsfId { id: "3a9f".into() }),
        (
            "launch_failed",
            Record::LaunchFailed {
                package: "com.example".into(),
                reason: "not installed".into(),
            },
        ),
        (
            "install_result",
            Record::InstallResult {
                path: "app.apk".into(),
                ok: true,
                message: String::new(),
            },
        ),
    ]
}

fn vectors() -> Vec<(&'static str, &'static str)> {
    VECTORS
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once(' '))
        .collect()
}

#[test]
fn every_record_matches_its_golden_vector() -> Result<(), WireError> {
    let vectors = vectors();
    let samples = samples();
    assert_eq!(vectors.len(), samples.len(), "one vector per record type");
    for ((name, record), (vector_name, hex_bytes)) in samples.iter().zip(vectors) {
        assert_eq!(*name, vector_name);
        assert_eq!(hex::encode(encode(record)?), hex_bytes, "{name}");
    }
    Ok(())
}

#[test]
fn every_record_round_trips() -> Result<(), WireError> {
    for (name, record) in samples() {
        let bytes = encode(&record)?;
        assert_eq!(
            decode(&bytes)?,
            Decoded::Record(record, bytes.len()),
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn a_partial_record_asks_for_more() -> Result<(), WireError> {
    let bytes = encode(&Record::Focus { task: 1 })?;
    for cut in 0..bytes.len() {
        assert_eq!(decode(&bytes[..cut])?, Decoded::Incomplete);
    }
    Ok(())
}

#[test]
fn trailing_or_missing_payload_bytes_are_malformed() -> Result<(), WireError> {
    let mut long = encode(&Record::Focus { task: 1 })?;
    long[0] += 1;
    long.push(0);
    assert_eq!(decode(&long), Err(WireError::Malformed(0x0102)));

    let short = [4, 0, 0, 0, 0x02, 0x01, 0, 0];
    assert_eq!(decode(&short), Err(WireError::Malformed(0x0102)));
    Ok(())
}

#[test]
fn unknown_types_and_huge_records_are_rejected() {
    assert_eq!(
        decode(&[4, 0, 0, 0, 0xFF, 0x7F, 0, 0]),
        Err(WireError::UnknownType(0x7FFF))
    );
    assert_eq!(decode(&[0, 0, 2, 0]), Err(WireError::TooLong(0x2_0000)));
}

#[test]
fn booleans_other_than_zero_or_one_are_malformed() -> Result<(), WireError> {
    let mut bytes = encode(&Record::Visibility {
        task: 1,
        visible: true,
    })?;
    let last = bytes.len() - 4;
    bytes[last] = 2;
    assert_eq!(decode(&bytes), Err(WireError::Malformed(0x0105)));
    Ok(())
}

/// Rewrites the vectors file after an intentional format change:
/// `cargo test -p crownos-android-agent-wire -- --ignored regenerate`.
#[test]
#[ignore = "writes contracts/agent-wire-vectors.txt"]
fn regenerate_vectors() -> Result<(), Box<dyn std::error::Error>> {
    let mut text = String::from(
        "# crown-agent wire v1 golden vectors: <name> <hex of the whole record>.\n\
         # Generated by crates/agent-wire (regenerate_vectors). The Java agent and the\n\
         # C++ hwcomposer must encode the same samples to these exact bytes.\n",
    );
    for (name, record) in samples() {
        text.push_str(&format!("{name} {}\n", hex::encode(encode(&record)?)));
    }
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/agent-wire-vectors.txt"
    );
    std::fs::write(path, text)?;
    Ok(())
}
