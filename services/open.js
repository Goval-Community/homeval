// TODO: actually implement this service
// It is used on replit when xdg-open is used
// to make a connected client open the files
// It only sends messages never recv's them so
// a blank service impl is fine.

class Service extends ServiceBase {
	async recv(_cmd, _session) {}
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
