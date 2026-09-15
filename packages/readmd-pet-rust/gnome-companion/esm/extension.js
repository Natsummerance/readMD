/* GNOME Shell >= 45 companion for ReadMD's native pet host. */
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';

export default class ReadmdPetCompanion {
  constructor() {
    this._socket = null;
    this._connections = [];
    this._peerPids = new Map();
    this._appId = 'com.readmd.desktop';
    this._pid = 0;
    this._token = '';
    this._window = null;
  }
  enable() {
    const path = GLib.build_filenamev([GLib.get_user_runtime_dir(), 'readmd-pet-gnome.sock']);
    try { GLib.unlink(path); } catch (_) {}
    this._socket = new Gio.SocketService();
    this._socket.add_address(new Gio.UnixSocketAddress({ path }), Gio.SocketType.STREAM, Gio.SocketProtocol.DEFAULT, null);
    this._socket.connect('incoming', (_service, connection) => {
      this._connections.push(connection);
      this._peerPids.set(connection, this._peerPid(connection));
      this._read(connection);
      return true;
    });
    this._socket.start();
  }
  _read(connection) {
    try {
      const input = connection.get_input_stream();
      input.read_bytes_async(64 * 1024, GLib.PRIORITY_DEFAULT, null, (_stream, result) => {
        try {
          const bytes = input.read_bytes_finish(result);
          const data = bytes.toArray();
          if (!data.length) return;
          const text = new TextDecoder().decode(data);
          const peerPid = this._peerPids.get(connection) || 0;
          text.split('\n').forEach(line => { try { if (line.trim()) this._apply(JSON.parse(line), peerPid); } catch (_) {} });
          this._read(connection);
        } catch (_) {
          this._connections = this._connections.filter(item => item !== connection);
          this._peerPids.delete(connection);
        }
      });
    } catch (_) {}
  }
  _peerPid(connection) {
    try {
      const socket = connection.get_socket && connection.get_socket();
      const credentials = socket && socket.get_credentials && socket.get_credentials();
      const pid = credentials && credentials.get_unix_pid && credentials.get_unix_pid();
      return Number(pid) || 0;
    } catch (_) { return 0; }
  }
  _apply(message, peerPid) {
    if (!peerPid || !message || message.version !== 1 || message.app_id !== this._appId || !message.session_token) return;
    if (message.method === 'register') {
      const declaredPid = Number(message.params && message.params.pid) || 0;
      if (!declaredPid || declaredPid !== peerPid) return;
      this._token = String(message.session_token);
      this._pid = declaredPid;
      this._window = this._findWindow();
      this._setVisible(true);
      return;
    }
    if (!this._token || peerPid !== this._pid || message.session_token !== this._token) return;
    if (!this._window || this._window.get_pid() !== this._pid) this._window = this._findWindow();
    if (message.method === 'set-bounds') this._setBounds(message.params || {});
    else if (message.method === 'show') this._setVisible(true);
    else if (message.method === 'hide') this._setVisible(false);
    else if (message.method === 'set-opacity') this._setOpacity(message.params || {});
  }
  _findWindow() {
    if (!this._pid || !global.get_window_actors) return null;
    const actors = global.get_window_actors();
    for (const actor of actors) {
      const window = actor.get_meta_window();
      if (window && window.get_pid() === this._pid) return window;
    }
    return null;
  }
  _actor() {
    return this._window && this._window.get_compositor_private ? this._window.get_compositor_private() : null;
  }
  _setBounds(params) {
    if (!this._window) return;
    const x = Number(params.x), y = Number(params.y), width = Number(params.width), height = Number(params.height);
    if (![x, y, width, height].every(Number.isFinite) || width < 1 || height < 1) return;
    if (typeof this._window.move_resize_frame === 'function') this._window.move_resize_frame(true, Math.round(x), Math.round(y), Math.round(width), Math.round(height));
  }
  _setVisible(visible) {
    if (!this._window) this._window = this._findWindow();
    const actor = this._actor();
    if (actor) visible ? actor.show() : actor.hide();
    if (this._window) {
      if (typeof this._window.make_above === 'function') visible ? this._window.make_above() : this._window.unmake_above();
      if (typeof this._window.stick === 'function') visible ? this._window.stick() : this._window.unstick();
      if (typeof this._window.set_skip_taskbar === 'function') this._window.set_skip_taskbar(true);
    }
  }
  _setOpacity(params) {
    const actor = this._actor();
    if (!actor || typeof actor.set_opacity !== 'function') return;
    const opacity = Math.max(0, Math.min(1, Number(params.opacity)));
    if (Number.isFinite(opacity)) actor.set_opacity(Math.round(opacity * 255));
  }
  disable() { if (this._socket) this._socket.stop(); this._socket = null; this._connections = []; this._peerPids.clear(); this._window = null; this._token = ''; }
}
