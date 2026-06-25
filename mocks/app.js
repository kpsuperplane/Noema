const screens = Array.from(document.querySelectorAll(".screen"));
const tabs = Array.from(document.querySelectorAll(".mock-tab"));
const params = new URLSearchParams(location.search);
const screenParam = params.get("screen");

if (params.get("capture") === "1") {
  document.body.classList.add("capture-mode");
}

function setScreen(id) {
  const target = screens.find((screen) => screen.id === id) || screens[0];
  screens.forEach((screen) => screen.classList.toggle("active", screen === target));
  tabs.forEach((tab) => tab.classList.toggle("active", tab.dataset.screen === target.id));
  if (!screenParam && location.hash.slice(1) !== target.id) {
    history.replaceState(null, "", `#${target.id}`);
  }
}

tabs.forEach((tab) => {
  tab.addEventListener("click", () => setScreen(tab.dataset.screen));
});

window.addEventListener("hashchange", () => setScreen(location.hash.slice(1)));
setScreen(screenParam || location.hash.slice(1) || "first-chat");
