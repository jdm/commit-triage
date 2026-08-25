import { sendMessageToServer, goToCommit } from "./main.js";
import { dialogs } from "./dialog-registry.js";
import { filteredCommits } from "./search-dialog.js";
dialogs.push(wordCloudDialog);

// used by event handler attributes in index.html
window.updateWordCloud = updateWordCloud;

let wordCloudFirstTime = true;

export function openWordCloudDialog() {
    wordCloudDialog.showModal();
    if (wordCloudFirstTime) {
        wordCloudFirstTime = false;
        updateWordCloud();
    }
}
async function updateWordCloud() {
    const response = await fetch("/wordCloud");
    const json = await response.json();
    console.log(json);
    const pre = wordCloudDialog.querySelector("pre");
    pre.innerHTML = "";
    if ("Ok" in json) {
        for (const [word, entries] of json.Ok.words) {
            const filteredEntries = entries.filter(entry => filteredCommits.some(commit => commit.hash_number == entry.hash_number));
            const hiddenCount = entries.length - filteredEntries.length;
            if (hiddenCount > 0) {
                pre.append(`${word} [${filteredEntries.length} (+${hiddenCount})]\n`);
            } else {
                pre.append(`${word} [${entries.length}]\n`);
            }
            for (const entry of filteredEntries) {
                const a = document.createElement("a");
                a.addEventListener("click", event => {
                    event.preventDefault();
                    goToCommit(entry.hash_number);
                    wordCloudDialog.close();
                });
                a.textContent = entry.hash_number;
                a.href = entry.hash_number;
                pre.append(a, ` - ${entry.title}\n`);
            }
            pre.append(`\n`);
        }
    } else if ("Err" in json) {
        pre.append(`>>> error: ${json.Err}`);
    }
}
