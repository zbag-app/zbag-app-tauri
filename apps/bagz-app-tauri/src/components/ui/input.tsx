import * as React from "react";
import { cn } from "../../lib/utils";

type InputProps = React.InputHTMLAttributes<HTMLInputElement> & {
  "data-lpignore"?: string;
  "data-1p-ignore"?: string;
};

const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, type, ...props }, ref) => {
    const isPassword = type === "password";
    const autoComplete = props.autoComplete ?? (isPassword ? "new-password" : "off");
    const name = props.name ?? (isPassword ? "zstash-secret" : undefined);
    const dataLpIgnore = props["data-lpignore"] ?? (isPassword ? "true" : undefined);
    const dataOnePasswordIgnore = props["data-1p-ignore"] ?? (isPassword ? "true" : undefined);
    return (
      <input
        type={type}
        name={name}
        autoComplete={autoComplete}
        autoCorrect={props.autoCorrect ?? "off"}
        autoCapitalize={props.autoCapitalize ?? "off"}
        spellCheck={props.spellCheck ?? false}
        data-lpignore={dataLpIgnore}
        data-1p-ignore={dataOnePasswordIgnore}
        className={cn(
          "flex h-9 w-full rounded-none border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50",
          className
        )}
        ref={ref}
        {...props}
      />
    );
  }
);
Input.displayName = "Input";

export { Input };
