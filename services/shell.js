class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        this.pty = new PtyProcess(this.id, process.env.SHELL || "sh")
        this.pty.init().then(_ => {
            console.debug("shell pty obtained:", this.pty.id)
        })
    }
    
	async recv(cmd, session) {
		if (cmd.input) {
			await this.pty.write(cmd.input)
		}

        // console.log(cmd, this)
	}

    async attach(session) {
        await this.pty.add_session(session)
    }
}

console.log(serviceInfo);
const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
