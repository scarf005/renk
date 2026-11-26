pub mod outer {
    pub mod inner {
        pub(super) struct Point {
            pub(super) x: i32,
            pub(in crate::outer) y: i32,
        }

        pub(self) struct Pair(pub(super) i32, i32);

        pub(in crate::outer) enum Shape<T> {
            Dot(T),
            Line { from: T, to: T },
        }

        #[derive(Clone, Copy)]
        pub(super) union Bits {
            pub(super) byte: u8,
            flag: bool,
        }

        pub(in crate::outer) trait Area {
            fn area(&self) -> i32;
        }

        pub(super) mod deep {
            pub(in crate::outer) fn depth() -> i32 {
                3
            }
        }

        impl Point {
            pub(super) fn new(x: i32) -> Self {
                Point { x, y: x }
            }

            pub(in crate::outer) const ORIGIN: i32 = 0;
        }

        impl Area for Point {
            fn area(&self) -> i32 {
                self.x * self.y
            }
        }

        pub(super) unsafe fn raw() -> i32 {
            1
        }

        pub(super) const fn fixed() -> i32 {
            2
        }

        pub(crate) fn wide() -> i32 {
            // pub(super) struct Hidden {
            let text = "pub(super) struct S {";
            Pair(text.len() as i32, 0).0
        }
    }

    pub fn total() -> i32 {
        let point = inner::Point::new(2);
        let Bits = inner::Bits { byte: 1 };
        let shape: inner::Shape<i32> = inner::Shape::Line { from: 1, to: 2 };
        let picked = match shape {
            inner::Shape::Dot(at) => at,
            inner::Shape::Line { from, to } => from + to,
        };
        use inner::Area;
        point.area()
            + point.x
            + picked
            + inner::deep::depth()
            + unsafe { inner::raw() }
            + inner::fixed()
            + inner::wide()
            + unsafe { Bits.byte } as i32
            + inner::Point::ORIGIN
    }
}
