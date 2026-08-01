self.addEventListener("push", (event) => {
  let notification;
  try {
    const payload = event.data?.json();
    notification = payload?.web_push === 8030 ? payload.notification : null;
  } catch {
    notification = null;
  }
  if (!notification?.title) {
    notification = {
      title: "Noema",
      body: "Something needs your attention.",
      navigate: new URL("/", self.location.origin).href,
      silent: false
    };
  }
  event.waitUntil(self.registration.showNotification(notification.title, {
    body: notification.body,
    data: { navigate: notification.navigate },
    silent: notification.silent,
    tag: notification.tag
  }));
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const destination = event.notification.data?.navigate;
  if (!destination) return;
  event.waitUntil((async () => {
    const destinationUrl = new URL(destination, self.location.origin);
    if (destinationUrl.origin !== self.location.origin) return;
    const windows = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
    const existing = windows.find((client) => new URL(client.url).origin === destinationUrl.origin);
    if (existing) {
      await existing.navigate(destinationUrl.href);
      await existing.focus();
      return;
    }
    await self.clients.openWindow(destinationUrl.href);
  })());
});
