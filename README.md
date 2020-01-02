# ws1001

A Rust library for working with the WS1001 weather observer base station.

Currently only provides a `Stream` which discovers, connects to, and then
periodically queries for weather data.

## WS1001 protocol

The WS1001 communicates using marshalled C structs. These have been decoded over
time by others in the community. This library decodes the C structs and re-exports
them into more ergonomic Rust types.

To connect to the base station, a TCP listener is setup on port 6500 and a UDP
broadcast sent on port 6000. The base station will connect back to the broadcaster
and then accept queries over this connection. 

## References

- https://www.mail-archive.com/weewx-user@googlegroups.com/msg10441/HP1000-gs.py
- https://github.com/matthewwall/weewx-observer/blob/master/bin/user/observer.py
- http://www.wxforum.net/index.php?topic=31229.15;imode
- https://www.mail-archive.com/weewx-user@googlegroups.com/msg10441/HP1000-gs.py