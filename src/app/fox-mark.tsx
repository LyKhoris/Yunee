import type { SVGProps } from "react";

/**
 * The fox mark: a solid single-colour silhouette with the eyes and nose cut out,
 * so it reads as one shape and stays legible down to 16px. Inherits `currentColor`
 * — neutral in chrome, accent when it is the focus.
 *
 * Ported from Foxi (the archived predecessor) so Yunee starts in the same visual
 * language. The artboard is trimmed to the drawing, so a size class is the mark's
 * real height and it lines up on the grid it is given. Yunee's own mark is still
 * undecided — swap this one component when it lands.
 */
export function FoxMark({ title, ...props }: SVGProps<SVGSVGElement> & { title?: string }) {
  return (
    <svg
      viewBox="107 116 298 298"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      role={title ? "img" : undefined}
      aria-label={title}
      aria-hidden={title ? undefined : true}
      {...props}
    >
      <mask id="yunee-mark">
        <rect width="512" height="512" fill="black" />
        <g fill="white">
          <path d="M150 116 L214 212 L118 224 Z" />
          <path d="M362 116 L298 212 L394 224 Z" />
          <path d="M256 176 C332 176 382 238 382 300 C382 372 320 414 256 414 C192 414 130 372 130 300 C130 238 180 176 256 176 Z" />
        </g>
        <g fill="black">
          <circle cx="208" cy="288" r="17" />
          <circle cx="304" cy="288" r="17" />
          <path d="M256 328 L235 352 L277 352 Z" />
        </g>
      </mask>
      <rect width="512" height="512" fill="currentColor" mask="url(#yunee-mark)" />
    </svg>
  );
}
