class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        this.pty = new PtyProcess(this.id, process.env.SHELL || "sh")
        this.pty.init(this.clients).then(_ => {
            console.debug("shell pty obtained:", this.pty.id)
        })
        this.dead_ptys = []
    }

    async process_dead(_) {
        if (this.dead_ptys.includes(pty)) {return}
        this.dead_ptys.push(pty)

        this.pty = new PtyProcess(this.id, process.env.SHELL || "sh", [], {
            "REPLIT_GIT_ASKPASS_GODS_PLS_SEND_TO_RIGHT_SESSION_SHELL_TOKEN": this.id.toString()
        });
        
        await this.pty.init(this.clients)
        console.debug("shell pty obtained:", this.pty.id)
    }
    
	async recv(cmd, session) {
		if (cmd.input) {
			await this.pty.write(cmd.input)
		}
	}

    async attach(session) {
        await this.pty.add_session(session)
    }

    async detach(session) {
        await this.pty.remove_session(session)
    }
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
