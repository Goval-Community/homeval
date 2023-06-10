class Service extends ServiceBase {
	async recv(cmd, _session) {
		if (cmd.fsSnapshot) { // No handling needed as all changes are persisted to the local fs by the local fs, and ot writes instantly
			return api.Command.create({ok: {}})
		}
	}
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
