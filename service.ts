import { api } from "@replit/protocol";

class Chat extends ServiceBase {
	data: { history: { author: string; message: string }[] };

	public async recv(cmd: api.Command, session: number): Promise<void> {
		if (cmd.chatMessage) {
			this.send(
				cmd,
				this.clients.filter((arr_session) => arr_session !== session),
			);
		} else if (cmd.readdir) {
			api.Command.create({
				files: { files: [{ path: "test.txt" }, { path: "fake file" }] },
			});
		}
	}
}
