@rem
@rem Copyright 2015 the original author or authors.
@rem
@rem Licensed under the Apache License, Version 2.0 (the "License");
@rem you may not use this file except in compliance with the License.
@rem You may obtain a copy of the License at
@rem
@rem      https://www.apache.org/licenses/LICENSE-2.0
@rem
@rem Unless required by applicable law or agreed to in writing, software
@rem distributed under the License is distributed on an "AS IS" BASIS,
@rem WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
@rem See the License for the specific language governing permissions and
@rem limitations under the License.
@rem
@rem SPDX-License-Identifier: Apache-2.0
@rem

@if "%DEBUG%"=="" @echo off
@rem ##########################################################################
@rem
@rem  p2p-app startup script for Windows
@rem
@rem ##########################################################################

@rem Set local scope for the variables, and ensure extensions are enabled
setlocal EnableExtensions

set DIRNAME=%~dp0
if "%DIRNAME%"=="" set DIRNAME=.
@rem This is normally unused
set APP_BASE_NAME=%~n0
set APP_HOME=%DIRNAME%..

@rem Resolve any "." and ".." in APP_HOME to make it shorter.
for %%i in ("%APP_HOME%") do set APP_HOME=%%~fi

@rem Add default JVM options here. You can also use JAVA_OPTS and P2P_APP_OPTS to pass JVM options to this script.
set DEFAULT_JVM_OPTS=

@rem Find java.exe
if defined JAVA_HOME goto findJavaFromJavaHome

set JAVA_EXE=java.exe
%JAVA_EXE% -version >NUL 2>&1
if %ERRORLEVEL% equ 0 goto execute

echo. 1>&2
echo ERROR: JAVA_HOME is not set and no 'java' command could be found in your PATH. 1>&2
echo. 1>&2
echo Please set the JAVA_HOME variable in your environment to match the 1>&2
echo location of your Java installation. 1>&2

"%COMSPEC%" /c exit 1

:findJavaFromJavaHome
set JAVA_HOME=%JAVA_HOME:"=%
set JAVA_EXE=%JAVA_HOME%/bin/java.exe

if exist "%JAVA_EXE%" goto execute

echo. 1>&2
echo ERROR: JAVA_HOME is set to an invalid directory: %JAVA_HOME% 1>&2
echo. 1>&2
echo Please set the JAVA_HOME variable in your environment to match the 1>&2
echo location of your Java installation. 1>&2

"%COMSPEC%" /c exit 1

:execute
@rem Setup the command line

set CLASSPATH=%APP_HOME%\lib\p2p-app-1.0.0-SNAPSHOT.jar;%APP_HOME%\lib\p2p-cli-1.0.0-SNAPSHOT.jar;%APP_HOME%\lib\p2p-transfer-1.0.0-SNAPSHOT.jar;%APP_HOME%\lib\p2p-security-1.0.0-SNAPSHOT.jar;%APP_HOME%\lib\p2p-network-1.0.0-SNAPSHOT.jar;%APP_HOME%\lib\p2p-crypto-1.0.0-SNAPSHOT.jar;%APP_HOME%\lib\p2p-observability-1.0.0-SNAPSHOT.jar;%APP_HOME%\lib\p2p-core-1.0.0-SNAPSHOT.jar;%APP_HOME%\lib\logback-classic-1.5.15.jar;%APP_HOME%\lib\slf4j-api-2.0.16.jar;%APP_HOME%\lib\jna-platform-5.16.0.jar;%APP_HOME%\lib\jna-5.16.0.jar;%APP_HOME%\lib\micrometer-registry-prometheus-1.14.4.jar;%APP_HOME%\lib\micrometer-core-1.14.4.jar;%APP_HOME%\lib\picocli-codegen-4.7.6.jar;%APP_HOME%\lib\picocli-4.7.6.jar;%APP_HOME%\lib\logback-core-1.5.15.jar;%APP_HOME%\lib\micrometer-observation-1.14.4.jar;%APP_HOME%\lib\micrometer-commons-1.14.4.jar;%APP_HOME%\lib\HdrHistogram-2.2.2.jar;%APP_HOME%\lib\LatencyUtils-2.0.3.jar;%APP_HOME%\lib\prometheus-metrics-core-1.3.5.jar;%APP_HOME%\lib\prometheus-metrics-tracer-common-1.3.5.jar;%APP_HOME%\lib\prometheus-metrics-exposition-formats-1.3.5.jar;%APP_HOME%\lib\prometheus-metrics-exposition-textformats-1.3.5.jar;%APP_HOME%\lib\prometheus-metrics-model-1.3.5.jar;%APP_HOME%\lib\prometheus-metrics-config-1.3.5.jar


@rem Execute p2p-app
@rem endlocal doesn't take effect until after the line is parsed and variables are expanded
@rem which allows us to clear the local environment before executing the java command
endlocal & "%JAVA_EXE%" %DEFAULT_JVM_OPTS% %JAVA_OPTS% %P2P_APP_OPTS%  -classpath "%CLASSPATH%" com.p2p.app.P2PApplication %* & call :exitWithErrorLevel

:exitWithErrorLevel
@rem Use "%COMSPEC%" /c exit to allow operators to work properly in scripts
"%COMSPEC%" /c exit %ERRORLEVEL%
