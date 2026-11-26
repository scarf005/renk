#[allow(non_camel_case_types)]
pub mod c {
    pub struct sockaddr_in {
        pub sin_family: u16,
        pub sin_port: u16,
    }

    pub struct timespec {
        pub tv_sec: i64,
    }

    pub struct Wrapper {
        pub inner: sockaddr_in,
    }
}

pub struct Ports {
    pub ports: Vec<u16>,
    pub open: bool,
}

pub fn address(port: u16) -> u16 {
    // c::sockaddr_in {
    let text = "c::sockaddr_in {";
    let v = c::sockaddr_in {
        sin_family: 1,
        sin_port: port,
    };
    let short = c::sockaddr_in { sin_family: 2, sin_port: 3 };
    let wrapped = c::Wrapper {
        inner: c::sockaddr_in {
            sin_family: 4,
            sin_port: 5,
        },
    };
    let times = vec![self::c::timespec {
        tv_sec: 6,
    }];
    let time = self::c::timespec {
        tv_sec: 7,
    };
    v.sin_port
        + short.sin_port
        + wrapped.inner.sin_family
        + (times[0].tv_sec + time.tv_sec) as u16
        + text.len() as u16
}

pub fn conditions(ports: Ports) -> u16 {
    let mut total = 0;
    if ports.open {
        total += 1;
    }
    while ports.open {
        break;
    }
    let open = match ports.open {
        true => 1,
        false => 0,
    };
    for port in ports.ports {
        total += port;
    }
    total + open
}
