import { renderTerminalOutput } from "./ansi.js";

let updateGitShowTimer = null;

export function debouncedUpdateGitShow(text) {
    // after some delay, update the `git show` output.
    // rendering the `git show` output can be expensive.
    if (updateGitShowTimer != null) {
        clearTimeout(updateGitShowTimer);
        updateGitShowTimer = null;
    }
    updateGitShowTimer = setTimeout(() => updateGitShow(text), 50);
}

function updateGitShow(text) {
    updateGitShowTimer = null;
    renderTerminalOutput(gitShow, text);
}
