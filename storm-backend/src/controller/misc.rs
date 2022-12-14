macro_rules! is_variant {
    ($e:expr, $p:path) => {
        match $e {
            $p => true,
            _ => false,
        }
    };
}
macro_rules! is_tup_variant {
    ($e:expr, $p:path) => {
        match $e {
            $p(_) => true,
            _ => false,
        }
    };
}
macro_rules! extract_variant {
    ($e:expr, $p:path) => {
        match $e {
            $p(value) => value,
            _ => panic!("expected {}", stringify!($p)),
        }
    };
}
macro_rules! null_if {
    ($e:expr, $f:expr) => {
        match $e {
            Some(value) => value,
            None => $f,
        }
    };
}
