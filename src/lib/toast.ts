/**
 * Show a temporary toast message at the bottom-right of the screen.
 * Works from any window context (overlay, preferences, recording).
 */
export function showToast(message: string, duration = 3000): void {
  // Remove any existing toast
  const existing = document.getElementById("snapforge-toast");
  if (existing) existing.remove();

  const toast = document.createElement("div");
  toast.id = "snapforge-toast";
  toast.textContent = message;
  Object.assign(toast.style, {
    position: "fixed",
    bottom: "20px",
    right: "20px",
    background: "rgba(24, 24, 24, 0.92)",
    backdropFilter: "blur(10px)",
    color: "rgba(255, 255, 255, 0.9)",
    padding: "10px 18px",
    borderRadius: "8px",
    fontSize: "13px",
    fontFamily: "system-ui, -apple-system, sans-serif",
    fontWeight: "500",
    boxShadow: "0 4px 20px rgba(0, 0, 0, 0.4)",
    border: "1px solid rgba(255, 255, 255, 0.1)",
    zIndex: "99999",
    opacity: "0",
    transform: "translateY(10px)",
    transition: "opacity 0.3s, transform 0.3s",
    pointerEvents: "none",
  });

  document.body.appendChild(toast);

  // Trigger animation
  requestAnimationFrame(() => {
    toast.style.opacity = "1";
    toast.style.transform = "translateY(0)";
  });

  setTimeout(() => {
    toast.style.opacity = "0";
    toast.style.transform = "translateY(10px)";
    setTimeout(() => toast.remove(), 300);
  }, duration);
}
