# 🤝 Contributing to PurgeKit

First off, thank you for considering contributing to **PurgeKit**! 🎉 It's people like you that make PurgeKit the ultimate developer cleanup tool.

---

## 🧭 How Can I Contribute?

### 🐛 Reporting Bugs
- **Check if it has already been reported** by searching the [GitHub Issues](https://github.com/ThanhNguyxnOrg/PurgeKit/issues).
- If not, create a new issue. Be sure to include:
  - 🖥️ Your Windows OS version.
  - 📝 Steps to reproduce the bug.
  - ❌ Actual vs. Expected behavior.
  - 📷 Screenshots or console logs if applicable.

### 💡 Suggesting Enhancements
We are always looking to expand our coverage:
- **New Developer Tool Caches**: Got a new dev tool preset we missed?
- **Remnant heuristics**: Suggestions for better keyword-matching.
- Create an issue explaining the feature and why it would benefit developer environments.

### 🛠️ Pull Requests
1. **Fork** the repository and create your branch from `main`.
2. **Make your changes**:
   - For backend changes, write clean, performance-minded Rust.
   - For frontend changes, write Svelte 5 logic following our established Flat Ember Orange design system.
3. **Run quality checks**:
   - `cargo clippy` to check Rust code quality.
   - `npm run check` to check Svelte TypeScript validation.
   - `npm run build` to make sure it builds successfully.
4. **Submit a Pull Request** with a detailed explanation of your changes.

---

## 🎨 Coding Standards

### Backend (Rust)
- Format code using `cargo fmt`.
- Handle errors safely using the `anyhow` crate where appropriate.
- Avoid locking the UI: run filesystem-heavy operations concurrently using threadpools (`rayon`) or asynchronously.

### Frontend (Svelte 5)
- Use Svelte 5 **Runes** (`$state`, `$derived`, `$props`, `$bindable`) rather than legacy Svelte 4 reactivity.
- Adhere strictly to the flat **Ember Orange theme** tokens defined in `src/app.css`:
  - Accent color: `var(--color-accent)` / `bg-accent` / `text-accent`.
  - Borders: `border-border-default` (1px, flat).
  - Backgrounds: `bg-app-bg` for panels, `bg-surface-bg` for cards.
  - Bo-góc (Border Radius) maximum is `8px` (`rounded-lg`) for cards/buttons, `12px` (`rounded-xl`) for modals. Avoid `rounded-2xl`+.
- Avoid using gradient text, glassmorphism filters (`backdrop-blur`), or glow effects (`shadow-purple-500/10`).

---

## 📜 Code of Conduct
By participating in this project, you agree to abide by our standards. Be respectful, helpful, and welcoming to all contributors. Let's build something awesome together! 🚀
