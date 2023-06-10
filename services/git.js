// TODO: actually implement this service
// It is used on replit when xdg-open is used
// to make a connected client open the files
// It only sends messages never recv's them so
// a blank service impl is fine.

class Service extends ServiceBase {
	constructor(...args) {
		super(...args)

		this.send_reply = {}
	}

	async recv(cmd, _session) {
		if (cmd.replspaceApiGitHubToken) {
			const nonce = cmd.replspaceApiGitHubToken.nonce
			const token = cmd.replspaceApiGitHubToken.token

			await Deno.core.ops.op_replspace_reply(nonce, {githubTokenRes: token});
		} else if (cmd.replspaceApiCloseFile) {
			const nonce = cmd.replspaceApiCloseFile.nonce
			const send = this.send_reply[nonce];
			
			if (!send) {
				return
			}

			await Deno.core.ops.op_replspace_reply(nonce, {openFileRes: {}});
		} else {
			console.log("Unknown message:", cmd)
		}
	}

	async on_replspace(session, msg) {
		let real_session = session
		if (session === 0) {
			// Basically we have no clue who its for so just send it to someone /shrug
			real_session = this.clients[Math.floor(Math.random()*this.clients.length)]
		}

		if (msg.githubTokenReq) {
			const nonce = msg.githubTokenReq;

			await this.send(api.Command.create({replspaceApiGetGitHubToken: {nonce}}), real_session);
		} else if (msg.openFileReq) {
			// (file, wait for close, nonce)
			const path = msg.openFileReq[0];
			const wait_close = msg.openFileReq[1];
			const nonce = msg.openFileReq[2];

			const cmd = api.Command.create({
				replspaceApiOpenFile: {
					nonce,
					waitForClose: wait_close,
					file: path
				}
			});

			this.send_reply[nonce] = wait_close
			await this.send(cmd, real_session)
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
