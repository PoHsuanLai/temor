//! Reading the text the kernel writes under `/proc`.

/// A TCP socket in the listening state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ListenSocket {
    /// The port.
    pub port: u16,
    /// The socket's inode, which a process's open files name.
    pub inode: u64,
}

/// The listening sockets in a `/proc/net/tcp` or `/proc/net/tcp6` table. Lines that do not read
/// as rows (the header, anything cut short) are skipped.
pub fn parse_tcp_table(text: &str) -> Vec<ListenSocket> {
    text.lines().filter_map(parse_row).collect()
}

/// `LISTEN`, as the kernel writes the state column.
const LISTEN: &str = "0A";

fn parse_row(line: &str) -> Option<ListenSocket> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    // sl local rem st tx:rx tr:tm retrnsmt uid timeout inode ...
    let local = fields.get(1)?;
    let state = fields.get(3)?;
    let inode = fields.get(9)?;
    if *state != LISTEN {
        return None;
    }
    let (_, port) = local.rsplit_once(':')?;
    Some(ListenSocket {
        port: u16::from_str_radix(port, 16).ok()?,
        inode: inode.parse().ok()?,
    })
}

/// The parent of the process whose `/proc/<pid>/stat` text this is. The name is in brackets and
/// may itself contain spaces and brackets, so the fields are counted from the last bracket.
pub fn parse_ppid(stat: &str) -> Option<u32> {
    let (_, after) = stat.rsplit_once(')')?;
    // state ppid ...
    after.split_whitespace().nth(1)?.parse().ok()
}

/// The inode in a `/proc/<pid>/fd/<n>` link target, if it names a socket (`socket:[12345]`).
pub fn parse_socket_link(target: &str) -> Option<u64> {
    target
        .strip_prefix("socket:[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1 0000000000000000 100 0 0 10 0\n   1: 0100007F:D2A4 0100007F:1F90 01 00000000:00000000 00:00000000 00000000  1000        0 22222 1 0000000000000000 20 4 30 10 -1\n   2: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 777 1 0000000000000000 100 0 0 10 0\n";

    const TABLE6: &str = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 00000000000000000000000001000000:0BB8 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 424242 1 0000000000000000 100 0 0 10 0\n";

    #[test]
    fn only_listening_rows_are_read_in_both_tables() {
        assert_eq!(
            parse_tcp_table(TABLE),
            vec![
                ListenSocket {
                    port: 8080,
                    inode: 12345
                },
                ListenSocket {
                    port: 22,
                    inode: 777
                },
            ]
        );
        assert_eq!(
            parse_tcp_table(TABLE6),
            vec![ListenSocket {
                port: 3000,
                inode: 424242
            }]
        );
    }

    #[test]
    fn damaged_rows_are_skipped_not_fatal() {
        let text = "garbage\n 0: 0100007F:ZZZZ 0:0 0A 0:0 0:0 0 0 0 5\n   0: 0100007F:1F90 00000000:0000 0A 0:0 0:0 0 0 0 9\n";
        assert_eq!(
            parse_tcp_table(text),
            vec![ListenSocket {
                port: 8080,
                inode: 9
            }]
        );
    }

    #[test]
    fn parent_is_found_after_a_name_with_brackets_and_spaces() {
        let cases = [
            (
                "1234 (bash) S 1200 1234 1234 34816 1234 4194304 ...",
                Some(1200),
            ),
            ("55 (tmux: server) S 1 55 55 0 -1 0", Some(1)),
            ("56 (a) b) (c) R 77 56 56 0", Some(77)),
            ("no brackets here", None),
            ("9 (cut) S", None),
        ];
        for (stat, want) in cases {
            assert_eq!(parse_ppid(stat), want, "{stat}");
        }
    }

    #[test]
    fn only_socket_links_name_an_inode() {
        let cases = [
            ("socket:[12345]", Some(12345)),
            ("pipe:[12345]", None),
            ("/dev/pts/3", None),
            ("socket:[x]", None),
        ];
        for (target, want) in cases {
            assert_eq!(parse_socket_link(target), want, "{target}");
        }
    }
}
