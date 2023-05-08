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
			return api.Command.create({
				files: { files: [{ path: "test.txt" }, { path: "fake file" }] },
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
