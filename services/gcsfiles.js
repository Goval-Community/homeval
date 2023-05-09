class Service extends ServiceBase {
	async recv(cmd, session) {
		console.log(cmd);
		if (cmd.chatMessage) {
			this.send(
				cmd,
				this.clients.filter((arr_session) => arr_session !== session),
			);
		}
		if (cmd.readdir) {
			let files = []
			
			try {
				files = await Deno.core.ops.op_list_dir(cmd.readdir.path)
			} catch(err) {
				return api.Command.create({error: err.toString()})
			}

			return api.Command.create({
				files: { files: files.map(item => {return {path: item.path, type: !item.directory ? api.File.Type.FILE : api.File.Type.DIRECTORY}}) },
			});
		}
	}
}

console.log(serviceInfo);
const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service._recv();
