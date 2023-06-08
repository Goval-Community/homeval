class Service extends ServiceBase {
	constructor(...args) {
		super(...args)
		this.config = this.toolchainify(Deno.core.ops.op_get_dotreplit_config())
	}

	toolchainify(input) {
		let res = {entrypoint: input.entrypoint, languageServers: []}

		for (const [key, value] of Object.entries(input.languages ? input.languages : {})) {
			res.languageServers.push(
				{id: key, name: key, language: key, fileTypeAttrs: {filePattern: value.pattern}}
			);
		}

		return res
	}

	async recv(cmd, _session) {
		if (cmd.toolchainGetRequest) {
			return api.Command.create({toolchainGetResponse: {configs: this.config}})
		}
	}
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
