import { sendMessageToServer, goToCommit } from "./main.js";
import { dialogs } from "./dialog-registry.js";
dialogs.push(wordCloudDialog);

// used by event handler attributes in index.html
window.updateWordCloud = updateWordCloud;

let wordCloudFirstTime = true;

editorDialog.addEventListener("close", event => {
    console.log(event);
    sendMessageToServer({"Reload": null});
});
editorForm.addEventListener("submit", event => {
    console.log(event);
    sendMessageToServer({"SetLabel": input.value});
});

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
            pre.append(`[${entries.length}] ${word}\n`);
            for (const entry of entries) {
                const a = document.createElement("a");
                a.addEventListener("click", event => {
                    event.preventDefault();
                    goToCommit(entry.hash_number.slice(1));
                    wordCloudDialog.close();
                });
                a.textContent = entry.hash_number;
                a.href = `#`;
                pre.append(a, ` - ${entry.title}\n`);
            }
        }
    } else if ("Err" in json) {
        pre.append(`>>> error: ${json.Err}`);
    }
}
