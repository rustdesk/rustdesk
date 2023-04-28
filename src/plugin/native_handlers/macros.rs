#[macro_export]
macro_rules! return_if_not_method {
    ($call: ident, $prefix: ident) => {
        if $call.starts_with($prefix) {
            return None;
        }
    };
}

#[macro_export]
macro_rules! call_if_method {
    ($call: ident ,$method: literal, $block: block) => {
        if ($call != $method) {
            $block
        }
    };
}

#[macro_export]
macro_rules! define_method_prefix {
    ($prefix: literal) => {
        #[inline]
        fn method_prefix(&self) -> &'static str {
            $prefix
        }
    };
}
