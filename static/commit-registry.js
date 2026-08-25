import { renderSearchDialog } from "./search-dialog.js";

export let commits = null;
export let commitExt = null;

export function setCommits(newCommits) {
    commits = newCommits;
}

export function setCommitExt(newCommitExt) {
    commitExt = newCommitExt;
}

export async function fetchCommits() {
    // close() if open, to ensure dialog is on top
    loading.close();
    loading.showModal();

    const response = await fetch("/commits");
    setCommits(await response.json());
    console.log(commits);
    renderSearchDialog();

    loading.close();
}
