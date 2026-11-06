"use strict";

const copyStatus = document.querySelector(".copy-status");

for (const button of document.querySelectorAll("[data-copy]")) {
  const command = document.getElementById(button.dataset.copy);
  if (!command || !copyStatus || !navigator.clipboard?.writeText) continue;

  button.hidden = false;
  button.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(command.textContent);
      copyStatus.textContent = "Command copied. Run it from your download folder.";
    } catch {
      copyStatus.textContent = "Couldn’t copy automatically. Select the command and copy it manually.";
    }
  });
}
