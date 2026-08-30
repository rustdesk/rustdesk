#!/usr/bin/env python3
from pathlib import Path
import sys


if len(sys.argv) != 2:
    raise SystemExit("usage: strip-ohos-frb-core-event-impl.py <bridge_generated.rs>")

path = Path(sys.argv[1])
text = path.read_text()
generated_impl = """impl support::IntoDart for EventToUI {
    fn into_dart(self) -> support::DartAbi {
        match self {
            Self::Event(field0) => vec![0.into_dart(), field0.into_into_dart().into_dart()],
            Self::Rgba(field0) => vec![1.into_dart(), field0.into_into_dart().into_dart()],
            Self::Texture(field0, field1) => vec![
                2.into_dart(),
                field0.into_into_dart().into_dart(),
                field1.into_into_dart().into_dart(),
            ],
        }
        .into_dart()
    }
}
impl support::IntoDartExceptPrimitive for EventToUI {}
impl rust2dart::IntoIntoDart<EventToUI> for EventToUI {
    fn into_into_dart(self) -> Self {
        self
    }
}
"""

if text.count(generated_impl) != 1:
    raise SystemExit("expected exactly one generated EventToUI conversion implementation")

text = text.replace(
    generated_impl,
    "// EventToUI conversion is implemented by the authoritative Core crate.\n",
)
path.write_text(text)
