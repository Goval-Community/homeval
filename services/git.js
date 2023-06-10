// TODO: actually implement this service
// It is used on replit when xdg-open is used
// to make a connected client open the files
// It only sends messages never recv's them so
// a blank service impl is fine.

class Service extends ServiceBase {
	constructor(...args) {
		super(...args)
	}

	async recv(cmd, _session) {
		if (cmd.replspaceApiGitHubToken) {
			const nonce = cmd.replspaceApiGitHubToken.nonce
			const token = cmd.replspaceApiGitHubToken.token

			await Deno.core.ops.op_replspace_reply(nonce, {githubTokenRes: token});
		} else {
			console.log("Unknown message:", cmd)
		}
	}

	async on_replspace(session, msg) {
		if (msg.githubTokenReq) {
			let real_session = session
			const nonce = msg.githubTokenReq;
			if (session === 0) {
				// Basically we have no clue who its for so just send it to someone /shrug
				real_session = this.clients[Math.floor(Math.random()*this.clients.length)]
			}

			await this.send(api.Command.create({replspaceApiGetGitHubToken: {nonce}}), real_session);
		} else if (msg.openFileReq) {
			// TODO: implement
			console.warn("replspace open file is currently not implemented")
		} else {
			console.warn("Unknown replspace message:", msg)
		}
	}
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
