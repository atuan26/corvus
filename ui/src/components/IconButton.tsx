import type { ReactNode } from "react";
import { Button, Tooltip, TooltipTrigger, type ButtonProps } from "react-aria-components";

interface Props extends Omit<ButtonProps, "children" | "aria-label"> {
  /** Accessible name and tooltip text. */
  label: string;
  shortcut?: string;
  icon: ReactNode;
  /** Show the label next to the icon as well. */
  showLabel?: boolean;
}

/** Icon button whose tooltip (on hover or keyboard focus) names the action and its shortcut. */
export function IconButton({ label, shortcut, icon, showLabel, className, ...rest }: Props) {
  return (
    <TooltipTrigger delay={500} closeDelay={0}>
      <Button
        {...rest}
        aria-label={shortcut ? `${label} (${shortcut})` : label}
        className={`${showLabel ? "btn btn-icon-label" : "icon-btn"} ${className ?? ""}`}
      >
        <span className="btn-icon" aria-hidden>
          {icon}
        </span>
        {showLabel && <span>{label}</span>}
      </Button>
      <Tooltip className="tooltip" offset={6}>
        {label}
        {shortcut && <kbd>{shortcut}</kbd>}
      </Tooltip>
    </TooltipTrigger>
  );
}
