/**
 * Icon registry. Every icon is a list of SVG shapes drawn on a 24×24 grid with
 * a 1.75 stroke (outline style). Add an entry here and it becomes available as
 * `Icon({ name })` everywhere.
 */

export type Shape = readonly [tag: "path" | "circle" | "rect" | "ellipse", attrs: Record<string, string | number>];

const path = (d: string, extra: Record<string, string | number> = {}): Shape => ["path", { d, ...extra }];

export const ICONS = {
  sparkle: [
    path("M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"),
  ],
  sparkles: [
    path("M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"),
    path("M20 3v4"),
    path("M22 5h-4"),
    path("M4 17v2"),
    path("M5 18H3"),
  ],
  layers: [
    path("m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z"),
    path("m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"),
    path("m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"),
  ],
  chart: [path("M18 20V10"), path("M12 20V4"), path("M6 20v-6")],
  file: [
    path("M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"),
    path("M14 2v4a2 2 0 0 0 2 2h4"),
    path("M10 9H8"),
    path("M16 13H8"),
    path("M16 17H8"),
  ],
  download: [
    path("M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"),
    path("m7 10 5 5 5-5"),
    path("M12 15V3"),
  ],
  star: [
    path("M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"),
  ],
  code: [path("m16 18 6-6-6-6"), path("m8 6-6 6 6 6")],
  book: [
    path("M12 7v14"),
    path("M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z"),
  ],
  settings: [
    path("M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"),
    ["circle", { cx: 12, cy: 12, r: 3 }],
  ],
  bell: [
    path("M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"),
    path("M10.3 21a1.94 1.94 0 0 0 3.4 0"),
  ],
  crown: [
    path("M11.562 3.266a.5.5 0 0 1 .876 0L15.39 8.87a1 1 0 0 0 1.516.294L21.183 5.5a.5.5 0 0 1 .798.519l-2.834 10.246a1 1 0 0 1-.956.734H5.81a1 1 0 0 1-.957-.734L2.02 6.02a.5.5 0 0 1 .798-.519l4.276 3.664a1 1 0 0 0 1.516-.294z", { fill: "currentColor" }),
    path("M5.5 21h13"),
  ],
  "chevron-right": [path("m9 18 6-6-6-6")],
  "chevron-left": [path("m15 18-6-6 6-6")],
  "chevron-down": [path("m6 9 6 6 6-6")],
  "chevrons-up-down": [path("m7 15 5 5 5-5"), path("m7 9 5-5 5 5")],
  send: [
    path("M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z"),
    path("m21.854 2.147-10.94 10.939"),
  ],
  x: [path("M18 6 6 18"), path("m6 6 12 12")],
  check: [path("M20 6 9 17l-5-5")],
  "trend-up": [path("M7 17 17 7"), path("M7 7h10v10")],
  building: [
    ["rect", { x: 4, y: 2, width: 16, height: 20, rx: 2 }],
    path("M9 22v-4h6v4"),
    path("M8 6h.01"),
    path("M16 6h.01"),
    path("M12 6h.01"),
    path("M12 10h.01"),
    path("M12 14h.01"),
    path("M16 10h.01"),
    path("M16 14h.01"),
    path("M8 10h.01"),
    path("M8 14h.01"),
  ],
  coins: [
    ["circle", { cx: 8, cy: 8, r: 6 }],
    path("M18.09 10.37A6 6 0 1 1 10.34 18"),
    path("M7 6h1v4"),
    path("m16.71 13.88.7.71-2.82 2.82"),
  ],
  calendar: [
    path("M8 2v4"),
    path("M16 2v4"),
    ["rect", { x: 3, y: 4, width: 18, height: 18, rx: 2 }],
    path("M3 10h18"),
    path("M8 14h.01"),
    path("M12 14h.01"),
    path("M16 14h.01"),
    path("M8 18h.01"),
    path("M12 18h.01"),
  ],
  filter: [
    path("M10 20a1 1 0 0 0 .553.895l2 1A1 1 0 0 0 14 21v-7a2 2 0 0 1 .517-1.341L21.74 4.67A1 1 0 0 0 21 3H3a1 1 0 0 0-.742 1.67l7.225 7.989A2 2 0 0 1 10 14z"),
  ],
  columns: [
    ["rect", { x: 3, y: 3, width: 7, height: 7, rx: 1 }],
    ["rect", { x: 14, y: 3, width: 7, height: 7, rx: 1 }],
    ["rect", { x: 14, y: 14, width: 7, height: 7, rx: 1 }],
    ["rect", { x: 3, y: 14, width: 7, height: 7, rx: 1 }],
  ],
  database: [
    ["ellipse", { cx: 12, cy: 5, rx: 9, ry: 3 }],
    path("M3 5V19A9 3 0 0 0 21 19V5"),
    path("M3 12A9 3 0 0 0 21 12"),
  ],
  "external-link": [
    path("M15 3h6v6"),
    path("M10 14 21 3"),
    path("M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"),
  ],
  user: [
    path("M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"),
    ["circle", { cx: 12, cy: 7, r: 4 }],
  ],
  inbox: [
    path("M22 12h-6l-2 3h-4l-2-3H2"),
    path("M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"),
  ],
} as const satisfies Record<string, readonly Shape[]>;

export type IconName = keyof typeof ICONS;
