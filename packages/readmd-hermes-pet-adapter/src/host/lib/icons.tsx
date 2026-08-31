import type { CSSProperties } from 'react'

export type IconComponent = (props: { style?: CSSProperties }) => JSX.Element
const Symbol = ({ children, style }: { children: string; style?: CSSProperties }) => (
  <span aria-hidden="true" style={{ display: 'inline-flex', lineHeight: 1, ...style }}>{children}</span>
)
export const AlertCircle: IconComponent = props => <Symbol {...props}>!</Symbol>
export const Clock: IconComponent = props => <Symbol {...props}>◷</Symbol>
export const Mail: IconComponent = props => <Symbol {...props}>✉</Symbol>
