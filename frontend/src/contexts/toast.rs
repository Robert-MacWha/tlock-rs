use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
pub struct Toast {
    pub id: uuid::Uuid,
    pub message: String,
    pub kind: ToastKind,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Clone, Copy)]
pub struct ToastContext(Signal<Vec<Toast>>);

const TOAST_TIMEOUT_MS: u32 = 5000;

impl ToastContext {
    pub fn new(storage: Signal<Vec<Toast>>) -> Self {
        ToastContext(storage)
    }

    pub fn push(&self, message: impl Into<String>, kind: ToastKind) {
        let mut storage = self.0;
        let id = uuid::Uuid::new_v4();
        let msg = message.into();

        storage.write().push(Toast {
            id,
            message: msg,
            kind,
        });

        spawn(async move {
            gloo_timers::future::TimeoutFuture::new(TOAST_TIMEOUT_MS).await;
            storage.write().retain(|t| t.id != id);
        });
    }
}

pub fn use_toast() -> Signal<Vec<Toast>> {
    use_context::<ToastContext>().0
}

#[component]
pub fn toast_container() -> Element {
    let ctx = use_toast();

    rsx! {
        div { class: "toast toast-end toast-bottom z-100",
            for toast in ctx.read().iter() {
                div {
                    key: "{toast.id}",
                    class: match toast.kind {
                        ToastKind::Success => "alert alert-success",
                        ToastKind::Error => "alert alert-error",
                        _ => "alert alert-info",
                    },
                    span { "{toast.message}" }
                }
            }
        }
    }
}
