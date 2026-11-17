/**
 * E54.1: the webview never shows its own context menu (Back, Reload, Save as, Print, Emoji,
 * Import passwords, Inspect). One listener on the window cancels every `contextmenu` event:
 * a right-click, the Menu key and Shift+F10 all fire it, on WebView2, WKWebView and WebKitGTK
 * alike. The listener runs in the capture phase, so a component that stops the event's
 * propagation cannot let the webview's menu through. It only cancels the default action: the
 * event still reaches the components, and one that has its own menu (the Characters grid, the
 * Foundries sidebar) opens it as before. No other event is touched, so the developer tools of a
 * debug build stay on their keyboard shortcut (F12, Ctrl+Shift+I, Cmd+Option+I).
 */
export function blockWebviewMenu(target: Window = window): () => void {
  const onContextMenu = (e: Event) => e.preventDefault();
  target.addEventListener('contextmenu', onContextMenu, true);
  return () => target.removeEventListener('contextmenu', onContextMenu, true);
}
