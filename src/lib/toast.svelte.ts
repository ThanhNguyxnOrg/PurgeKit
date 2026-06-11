export interface ToastMessage {
    id: string;
    message: string;
    type: "success" | "error" | "info" | "warning";
}

let toasts = $state<ToastMessage[]>([]);

export const toast = {
    get toasts() { return toasts; },
    show(message: string, type: ToastMessage["type"] = "info", duration = 4000) {
        const id = Math.random().toString(36).substring(2, 9);
        toasts.push({ id, message, type });
        setTimeout(() => {
            this.remove(id);
        }, duration);
    },
    remove(id: string) {
        const idx = toasts.findIndex((t) => t.id === id);
        if (idx !== -1) {
            toasts.splice(idx, 1);
        }
    }
};
