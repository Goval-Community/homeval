class Service extends ServiceBase {
	constructor(...args) {
		super(...args)
		this.config = Deno.core.ops.op_get_dotreplit_config()
	}
	async recv(cmd, _session) {
		if (cmd.dotReplitGetRequest) {
			return api.Command.create({dotReplitGetResponse: {dotReplit: this.config}})
		}
	}
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
