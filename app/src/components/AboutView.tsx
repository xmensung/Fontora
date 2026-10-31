import { Fragment } from "react";
import { motion } from "motion/react";
import { spring } from "../design/springs";
import { useT } from "../lib/i18n";
import { APP_LICENSE, APP_VERSION } from "../lib/version";

const STACK = ["Tauri v2", "Rust", "React 19", "TypeScript", "Vite", "pnpm"];

export function AboutView() {
  const t = useT();
  const lines = t("about.desc").split("\n");

  return (
    <motion.div
      className="about-view"
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      transition={spring}
    >
      <img className="about-icon" src="/icon-512.png" alt="" draggable={false} />
      <h1 className="about-name">Fontora</h1>
      <div className="about-version tabular">
        {t("about.version", { version: APP_VERSION, license: APP_LICENSE })}
      </div>
      <p className="about-desc">
        {lines.map((line, i) => (
          <Fragment key={i}>
            {i > 0 && <br />}
            {line}
          </Fragment>
        ))}
      </p>

      <div className="about-stack" aria-label={t("about.stackAria")}>
        {STACK.map((item) => (
          <span key={item} className="about-chip tabular">
            {item}
          </span>
        ))}
      </div>
    </motion.div>
  );
}
