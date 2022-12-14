from flask import Flask

if __name__ == "__main__":

    app = Flask("example")

    @app.route('/')
    def index():
        return "<h1>Tor works!</h1>"
    app.run(host='127.0.0.1', port=8002)
