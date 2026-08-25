import { sendMessageToServer, goToCommit } from "./main.js";
import { commitExt } from "./commit-registry.js";
import { dialogs } from "./dialog-registry.js";
dialogs.push(gotoDialog);

gotoDialog.addEventListener("close", event => {
    console.log(event);
    sendMessageToServer({"Reload": null});
});
gotoForm.addEventListener("submit", event => {
    console.log(event);
    goToCommit(gotoInput.value);
});

export function openGotoDialog() {
    gotoDialog.showModal();
    gotoInput.value = commitExt.hash_number.slice(1);
    gotoInput.select();
}
