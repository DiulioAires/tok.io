import chatgptLogo from "../../assets/Chatgptlogo.svg?raw";
import claudeLogo from "../../assets/Claudelogo.svg?raw";

type ProviderIconProps = { provider: "openai" | "claude"; compact?: boolean };

export default function ProviderIcon({ provider, compact = false }: ProviderIconProps) {
  return (
    <span
      className={`provider-icon provider-icon-${provider} ${compact ? "is-compact" : ""}`}
      aria-label={provider === "claude" ? "Claude" : "ChatGPT"}
      role="img"
    >
      <span
        className="provider-icon-art"
        aria-hidden="true"
        dangerouslySetInnerHTML={{ __html: provider === "claude" ? claudeLogo : chatgptLogo }}
      />
    </span>
  );
}
