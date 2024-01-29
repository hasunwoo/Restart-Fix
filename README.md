# Restart-Fix

Simple tool built for detecting unintended restarts of windows computer built for personal needs.
If unintended restarts are detected, this program will initiates shutdown.

개인적인 필요에 의해 만들어진 윈도우 컴퓨터의 비정상적인 재시작을 감지하기 위한 도구. 비정상적인 재시작이 감지되면, 컴퓨터를 종료한다.

# Configuration(in [main.rs](src/main.rs?plain=1#L28) file)

**[TRESHOLD:](src/main.rs?plain=1#L28)** Define a threshold duration used to determine if the system should initiate a shutdown sequence.

**[SHUTDOWN_TIMEOUT:](src/main.rs?plain=1#L35)** Specify the timeout duration for the shutdown process. If the user does not cancel the shutdown within this timeframe, the system will proceed to shut down.
