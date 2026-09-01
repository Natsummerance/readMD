interface Window {
  readmdPet?: { dropFiles: (files: File[]) => void }
  hermesDesktop?: {
    onWindowStateChanged?: (callback: (payload: { isMinimized?: boolean; isVisible?: boolean }) => void) => () => void
    petOverlay?: {
      open: (request: unknown) => Promise<{ ok?: boolean; bounds?: unknown }>
      close: () => Promise<{ ok?: boolean }>
      setBounds: (bounds: unknown) => void
      setIgnoreMouse: (ignore: boolean) => void
      setFocusable: (focusable: boolean) => void
      pushState: (payload: unknown) => void
      control: (payload: unknown) => void
      onState: (callback: (payload: any) => void) => (() => void)
      onControl: (callback: (payload: any) => void) => (() => void)
    }
  }
}
