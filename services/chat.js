class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        this.history = [];
    }

	async recv(cmd, session) {
        if (cmd.chatMessage) {
            this.history.push(cmd.chatMessage)
            this.send({chatMessage: cmd.chatMessage}, -session)
        } else if (cmd.chatTyping) {
            this.send({chatTyping: cmd.chatTyping}, -session)
        }
    }

    async attach(session) {
        this.send({chatScrollback: {scrollback:this.history}}, session)
    }
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
