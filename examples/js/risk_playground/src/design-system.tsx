// Copyright The Pit Project Owners. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Please see https://openpit.dev and the OWNERS file for details.

import { useState } from "react";

import type {
  ButtonHTMLAttributes,
  CSSProperties,
  HTMLAttributes,
} from "react";

const BADGE_VARIANTS = {
  danger: {
    background: "var(--accent-dim)",
    borderColor: "var(--danger)",
    color: "var(--danger)",
  },
  ok: {
    background: "var(--accent-dim)",
    borderColor: "var(--ok)",
    color: "var(--ok)",
  },
} satisfies Record<string, CSSProperties>;

type BadgeProps = HTMLAttributes<HTMLSpanElement> & {
  readonly variant: keyof typeof BADGE_VARIANTS;
};

/** Compact status badge used by the playground account ribbon. */
export function Badge({ children, style, variant, ...rest }: BadgeProps) {
  return (
    <span
      style={{
        alignItems: "center",
        border: "1px solid",
        borderRadius: "var(--radius-sm)",
        display: "inline-flex",
        fontFamily: "var(--font-mono)",
        fontSize: "var(--text-xs)",
        fontWeight: "var(--weight-medium)",
        gap: 6,
        letterSpacing: "var(--tracking-label)",
        lineHeight: 1.4,
        padding: "2px 8px",
        textTransform: "uppercase",
        ...BADGE_VARIANTS[variant],
        ...style,
      }}
      {...rest}
    >
      {children}
    </span>
  );
}

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  readonly size?: "default" | "sm" | "icon";
  readonly variant?: "default" | "outline" | "ghost";
};

/** Dense playground action button. */
export function Button({
  children,
  disabled = false,
  size = "default",
  style,
  type = "button",
  variant = "default",
  ...rest
}: ButtonProps) {
  const [hovered, setHovered] = useState(false);
  const sizes: Readonly<
    Record<NonNullable<ButtonProps["size"]>, CSSProperties>
  > = {
    default: { fontSize: "var(--text-base)", height: 36, padding: "0 16px" },
    icon: { height: 36, padding: 0, width: 36 },
    sm: { fontSize: "var(--text-xs)", height: 32, padding: "0 12px" },
  };
  const variants: Readonly<
    Record<NonNullable<ButtonProps["variant"]>, CSSProperties>
  > = {
    default: {
      background: hovered && !disabled ? "var(--accent-2)" : "var(--accent)",
      color: "var(--bg)",
    },
    ghost: {
      background: hovered && !disabled ? "var(--accent-dim)" : "transparent",
      color: hovered && !disabled ? "var(--accent)" : "var(--muted-lt)",
    },
    outline: {
      background: "transparent",
      borderColor:
        hovered && !disabled ? "var(--border-hover)" : "var(--border)",
      color: hovered && !disabled ? "var(--accent)" : "var(--text)",
    },
  };

  return (
    <button
      disabled={disabled}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        alignItems: "center",
        border: "1px solid transparent",
        borderRadius: "var(--radius-card)",
        cursor: disabled ? "not-allowed" : "pointer",
        display: "inline-flex",
        fontFamily: "var(--font-mono)",
        fontWeight: "var(--weight-medium)",
        gap: 8,
        justifyContent: "center",
        opacity: disabled ? 0.5 : 1,
        transition:
          "background var(--dur-base) var(--ease), border-color var(--dur-base) var(--ease), color var(--dur-base) var(--ease)",
        whiteSpace: "nowrap",
        ...sizes[size],
        ...variants[variant],
        ...style,
      }}
      type={type}
      {...rest}
    >
      {children}
    </button>
  );
}

/** Bordered surface for a discrete playground panel. */
export function Card({
  children,
  style,
  ...rest
}: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      style={{
        background: "var(--surface)",
        border: "1px solid var(--border)",
        borderRadius: "var(--radius-card)",
        boxShadow: "var(--shadow-card)",
        color: "var(--text)",
        ...style,
      }}
      {...rest}
    >
      {children}
    </div>
  );
}
