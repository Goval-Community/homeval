class Service extends ServiceBase {
	async recv(_cmd, _session) {}
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
